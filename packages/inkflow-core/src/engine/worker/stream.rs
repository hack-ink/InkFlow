use std::sync::Arc;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{error::AppError, settings::SttSettings, stt};

use crate::engine::{AsrUpdate, queue::SecondPassQueue, state::{SegmentState, WindowState}};
use super::audio::{SpeechActivity, WhisperJob, ms_to_samples};
use super::second_pass::SecondPassScheduler;

struct StreamWorker {
	cancel: CancellationToken,
	stt_settings: SttSettings,
	recognizer: sherpa_onnx::OnlineRecognizer,
	stream: sherpa_onnx::OnlineStream,
	sample_rate: u32,
	engine_generation: u64,
	audio_rx: mpsc::Receiver<Vec<f32>>,
	update_tx: mpsc::Sender<AsrUpdate>,
	second_pass_queue: Arc<SecondPassQueue>,
	speech_activity: Arc<SpeechActivity>,
	window_tx: std::sync::mpsc::SyncSender<WhisperJob>,
	window_state: WindowState,
	segment_state: SegmentState,
	last_text: String,
	samples_per_read: usize,
	second_pass: SecondPassScheduler,
	last_window_backpressure: bool,
}

impl StreamWorker {
	fn run(mut self) -> Result<(), AppError> {
		let mut pending: Vec<f32> = Vec::new();
		let mut pending_start: usize = 0;

		while let Some(chunk) = self.audio_rx.blocking_recv() {
			if self.cancel.is_cancelled() {
				return Ok(());
			}

			pending.extend_from_slice(&chunk);

			while pending.len().saturating_sub(pending_start) >= self.samples_per_read {
				let end = pending_start.saturating_add(self.samples_per_read);
				self.process_samples(&pending[pending_start..end])?;
				pending_start = end;

				if pending_start >= 8_192 && pending_start >= pending.len().saturating_div(2) {
					pending.drain(..pending_start);
					pending_start = 0;
				}
			}
		}

		if pending_start < pending.len() {
			self.process_samples(&pending[pending_start..])?;
		}

		if self.cancel.is_cancelled() {
			return Ok(());
		}

		self.finalize_stream()?;
		Ok(())
	}

	fn process_samples(&mut self, samples: &[f32]) -> Result<(), AppError> {
		const SHERPA_SAMPLE_RATE_HZ: u32 = 16_000;

		if self.cancel.is_cancelled() || samples.is_empty() {
			return Ok(());
		}

		self.append_pending_tail(samples);

		let samples_16k;
		let samples_for_sherpa: &[f32] = if self.sample_rate == SHERPA_SAMPLE_RATE_HZ {
			samples
		} else {
			samples_16k = stt::resample_linear_to_16k(samples, self.sample_rate);
			samples_16k.as_slice()
		};

		self.stream.accept_waveform(SHERPA_SAMPLE_RATE_HZ as i32, samples_for_sherpa);
		self.segment_state.push_samples(samples);
		self.window_state.push_samples(samples_for_sherpa);

		self.recognizer.decode(&self.stream);

		let result = self.recognizer.result_json(&self.stream).map_err(|err| {
			AppError::new(
				"stt_decode_failed",
				format!("Failed to decode audio with sherpa-onnx: {err}."),
			)
		})?;

		let text = result.text.trim().to_string();
		self.maybe_emit_partial(&text);

		let queue_len = self.second_pass_queue.len();
		let allow_windows = queue_len < self.stt_settings.window.window_backpressure_high_watermark;
		let backpressure = !allow_windows;
		if backpressure != self.last_window_backpressure {
			if backpressure {
				tracing::warn!(
					queue_len,
					"Window decoding suppressed due to second-pass backpressure."
				);
			} else {
				tracing::info!("Window decoding resumed after backpressure.");
			}
			self.last_window_backpressure = backpressure;
		}
		for (snapshot, audio_16k) in self.window_state.drain_ready_jobs(
			self.engine_generation,
			!self.last_text.is_empty(),
			allow_windows,
		) {
			if self
				.window_tx
				.try_send(WhisperJob::Window { snapshot: snapshot.clone(), audio_16k })
				.is_ok()
			{
				let _ = self.update_tx.blocking_send(AsrUpdate::WindowScheduled(snapshot));
			}
		}

		if self.stream.is_endpoint() {
			let sherpa_text = if text.is_empty() { self.last_text.clone() } else { text };
			self.handle_endpoint(&sherpa_text)?;
		}

		Ok(())
	}

	fn maybe_emit_partial(&mut self, text: &str) {
		if text.is_empty() || text == self.last_text {
			return;
		}

		self.speech_activity.mark();
		self.last_text = text.to_string();

		let has_voice = self.segment_state.peak_mean_abs() >= self.stt_settings.window.min_mean_abs;
		if !has_voice {
			return;
		}

		let _ = self.update_tx.blocking_send(AsrUpdate::SherpaPartial(self.last_text.clone()));
	}

	fn handle_endpoint(&mut self, sherpa_text: &str) -> Result<(), AppError> {
		self.flush_pending_second_pass(true);
		let has_voice = self.segment_state.peak_mean_abs() >= self.stt_settings.window.min_mean_abs;
		let window_generation_after = self.window_state.advance_generation();

		if !has_voice || sherpa_text.trim().is_empty() {
			let _ =
				self.update_tx.blocking_send(AsrUpdate::EndpointReset { window_generation_after });
			self.segment_state.reset();
			self.last_text.clear();
			self.stream.reset();
			return Ok(());
		}

		let segment_id = self.segment_state.next_segment_id();
		let _ = self.update_tx.blocking_send(AsrUpdate::SegmentEnd {
			segment_id,
			sherpa_text: sherpa_text.to_string(),
			committed_end_16k_samples: self.window_state.total_16k_samples(),
			window_generation_after,
		});

		let (segment_samples, peak_mean_abs) = self.segment_state.take();
		tracing::info!(
			segment_id,
			samples = segment_samples.len(),
			peak_mean_abs,
			"Segment committed."
		);
		self.schedule_second_pass(segment_id, segment_samples, peak_mean_abs);

		self.last_text.clear();
		self.stream.reset();
		Ok(())
	}

	fn finalize_stream(&mut self) -> Result<(), AppError> {
		const SHERPA_SAMPLE_RATE_HZ: u32 = 16_000;
		const TAIL_PADDING_MS: u64 = 300;

		let tail_samples = (self.sample_rate as u64)
			.saturating_mul(TAIL_PADDING_MS)
			.saturating_div(1_000) as usize;
		if tail_samples > 0 {
			let tail = vec![0.0f32; tail_samples];
			let tail_16k_samples = (SHERPA_SAMPLE_RATE_HZ as u64)
				.saturating_mul(TAIL_PADDING_MS)
				.saturating_div(1_000) as usize;
			if tail_16k_samples > 0 {
				let tail_16k = vec![0.0f32; tail_16k_samples];
				self.stream.accept_waveform(SHERPA_SAMPLE_RATE_HZ as i32, &tail_16k);
			}
			self.segment_state.push_samples(&tail);
		}

		self.stream.input_finished();
		self.recognizer.decode(&self.stream);

		let result = self.recognizer.result_json(&self.stream).map_err(|err| {
			AppError::new(
				"stt_decode_failed",
				format!("Failed to decode audio with sherpa-onnx: {err}."),
			)
		})?;

		let fallback_text = forced_finalize_fallback_text(&result.text, &self.last_text);

		if self.segment_state.is_empty() {
			return Ok(());
		}

		let has_voice = self.segment_state.peak_mean_abs() >= self.stt_settings.window.min_mean_abs;
		let window_generation_after = self.window_state.advance_generation();

		if !has_voice || fallback_text.trim().is_empty() {
			let _ =
				self.update_tx.blocking_send(AsrUpdate::EndpointReset { window_generation_after });
			return Ok(());
		}

		let segment_id = self.segment_state.next_segment_id();
		let _ = self.update_tx.blocking_send(AsrUpdate::SegmentEnd {
			segment_id,
			sherpa_text: fallback_text,
			committed_end_16k_samples: self.window_state.total_16k_samples(),
			window_generation_after,
		});

		let (segment_samples, peak_mean_abs) = self.segment_state.take();
		tracing::info!(
			segment_id,
			samples = segment_samples.len(),
			peak_mean_abs,
			"Segment committed."
		);
		self.schedule_second_pass(segment_id, segment_samples, peak_mean_abs);
		self.flush_pending_second_pass(true);

		Ok(())
	}

	fn append_pending_tail(&mut self, samples: &[f32]) {
		if let Some(job) = self.second_pass.append_tail(samples)
			&& let WhisperJob::SecondPass { segment_id, sample_rate_hz, samples, peak_mean_abs } =
				job
			{
				self.enqueue_second_pass(segment_id, sample_rate_hz, samples, peak_mean_abs);
			}
	}

	fn schedule_second_pass(
		&mut self,
		segment_id: u64,
		segment_samples: Vec<f32>,
		peak_mean_abs: f32,
	) {
		self.flush_pending_second_pass(true);
		let tail_ms = self.stt_settings.window.endpoint_tail_ms;
		let tail_samples = ms_to_samples(self.sample_rate, tail_ms);
		if let Some(job) = self.second_pass.schedule(
			segment_id,
			self.sample_rate,
			segment_samples,
			peak_mean_abs,
			tail_samples,
		)
			&& let WhisperJob::SecondPass { segment_id, sample_rate_hz, samples, peak_mean_abs } =
				job
			{
				self.enqueue_second_pass(segment_id, sample_rate_hz, samples, peak_mean_abs);
			}
	}

	fn flush_pending_second_pass(&mut self, force: bool) {
		if let Some(job) = self.second_pass.flush(force)
			&& let WhisperJob::SecondPass { segment_id, sample_rate_hz, samples, peak_mean_abs } =
				job
			{
				self.enqueue_second_pass(segment_id, sample_rate_hz, samples, peak_mean_abs);
			}
	}

	fn enqueue_second_pass(
		&self,
		segment_id: u64,
		sample_rate_hz: u32,
		samples: Vec<f32>,
		peak_mean_abs: f32,
	) {
		let accepted = self.second_pass_queue.push(WhisperJob::SecondPass {
			segment_id,
			sample_rate_hz,
			samples,
			peak_mean_abs,
		});
		if accepted {
			tracing::debug!(segment_id, "Second-pass enqueued.");
		} else {
			tracing::warn!(segment_id, "Second-pass enqueue dropped.");
		}
	}
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_asr_worker(
	handle: &tokio::runtime::Handle,
	cancel: CancellationToken,
	stt_settings: SttSettings,
	recognizer: sherpa_onnx::OnlineRecognizer,
	stream: sherpa_onnx::OnlineStream,
	sample_rate: u32,
	engine_generation: u64,
	audio_rx: mpsc::Receiver<Vec<f32>>,
	update_tx: mpsc::Sender<AsrUpdate>,
	second_pass_queue: Arc<SecondPassQueue>,
	speech_activity: Arc<SpeechActivity>,
	window_tx: std::sync::mpsc::SyncSender<WhisperJob>,
	window_enabled: bool,
) -> tokio::task::JoinHandle<Result<(), AppError>> {
	handle.spawn_blocking(move || -> Result<(), AppError> {
		let chunk_ms = stt_settings.sherpa.chunk_ms as u64;
		let samples_per_read =
			(sample_rate as u64).saturating_mul(chunk_ms).saturating_div(1_000).max(1) as usize;

		let window_state = WindowState::new(&stt_settings, window_enabled);
		let segment_state = SegmentState::new();

		let worker = StreamWorker {
			cancel,
			stt_settings,
			recognizer,
			stream,
			sample_rate,
			engine_generation,
			audio_rx,
			update_tx,
			second_pass_queue,
			speech_activity,
			window_tx,
			window_state,
			segment_state,
			last_text: String::new(),
			samples_per_read,
			second_pass: SecondPassScheduler::new(),
			last_window_backpressure: false,
		};

		worker.run()
	})
}

fn forced_finalize_fallback_text(final_text: &str, last_text: &str) -> String {
	let trimmed = final_text.trim();
	if trimmed.is_empty() {
		last_text.trim().to_string()
	} else {
		trimmed.to_string()
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn forced_finalize_fallback_text_uses_last_text_when_final_empty() {
		let resolved = super::forced_finalize_fallback_text("", "hello");
		assert_eq!(resolved, "hello");
	}
}
