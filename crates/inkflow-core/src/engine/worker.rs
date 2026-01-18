use std::{sync::Arc, time::Duration};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{domain, error::AppError, settings::SttSettings, stt};

use super::{
	AsrUpdate,
	state::{SegmentState, WindowState},
};

#[derive(Debug)]
pub(crate) enum WhisperJob {
	SecondPass { segment_id: u64, sample_rate_hz: u32, samples: Vec<f32>, peak_mean_abs: f32 },
	Window { snapshot: stt::WindowJobSnapshot, audio_16k: Vec<f32> },
}

struct WhisperWorker {
	cancel: CancellationToken,
	whisper_config: stt::WhisperConfig,
	whisper_ctx: Arc<whisper_rs::WhisperContext>,
	stt_settings: SttSettings,
	second_pass_profile: stt::WhisperDecodeProfile,
	window_profile: stt::WhisperDecodeProfile,
	update_tx: mpsc::Sender<AsrUpdate>,
	second_pass_rx: std::sync::mpsc::Receiver<WhisperJob>,
	window_rx: std::sync::mpsc::Receiver<WhisperJob>,
}

impl WhisperWorker {
	fn run(self) {
		let min_mean_abs = self.stt_settings.window.min_mean_abs;
		let window_activity_ms = self.stt_settings.window.step_ms.clamp(80, 600);
		let window_activity_samples_16k = domain::ms_to_samples_16k(window_activity_ms) as usize;

		loop {
			if self.cancel.is_cancelled() {
				return;
			}

			self.drain_second_pass(min_mean_abs);

			if self.cancel.is_cancelled() {
				return;
			}

			match self.window_rx.recv_timeout(Duration::from_millis(20)) {
				Ok(WhisperJob::Window { snapshot, audio_16k }) => {
					if self.cancel.is_cancelled() {
						return;
					}

					if audio_16k.is_empty() {
						continue;
					}

					let start = audio_16k.len().saturating_sub(window_activity_samples_16k.max(1));
					let mean_abs = super::mean_abs(&audio_16k[start..]);
					if mean_abs < min_mean_abs {
						continue;
					}

					match stt::transcribe_segments(
						self.whisper_ctx.as_ref(),
						&audio_16k,
						&self.whisper_config,
						self.window_profile,
					) {
						Ok(result) => {
							let _ = self
								.update_tx
								.blocking_send(AsrUpdate::WindowResult { snapshot, result });
						},
						Err(err) => {
							eprintln!("Whisper window transcription failed: {}.", err.message);
						},
					}
				},
				Ok(WhisperJob::SecondPass { .. }) => {},
				Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {},
				Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
			}
		}
	}

	fn drain_second_pass(&self, min_mean_abs: f32) {
		while let Ok(job) = self.second_pass_rx.try_recv() {
			if self.cancel.is_cancelled() {
				return;
			}

			let WhisperJob::SecondPass { segment_id, sample_rate_hz, samples, peak_mean_abs } = job
			else {
				continue;
			};

			if samples.is_empty() {
				continue;
			}

			if peak_mean_abs < min_mean_abs {
				continue;
			}

			let audio_16k = stt::resample_linear_to_16k(&samples, sample_rate_hz);
			match stt::transcribe(
				self.whisper_ctx.as_ref(),
				&audio_16k,
				&self.whisper_config,
				self.second_pass_profile,
			) {
				Ok(text) => {
					let text = text.trim().to_string();
					if text.is_empty() {
						continue;
					}
					let _ =
						self.update_tx.blocking_send(AsrUpdate::SecondPass { segment_id, text });
				},
				Err(err) => {
					eprintln!(
						"Whisper second-pass transcription failed for segment {}: {}.",
						segment_id, err.message
					);
				},
			}
		}
	}
}

struct StreamWorker {
	cancel: CancellationToken,
	stt_settings: SttSettings,
	recognizer: sherpa_onnx::OnlineRecognizer,
	stream: sherpa_onnx::OnlineStream,
	sample_rate: u32,
	engine_generation: u64,
	audio_rx: mpsc::Receiver<Vec<f32>>,
	update_tx: mpsc::Sender<AsrUpdate>,
	second_pass_tx: std::sync::mpsc::Sender<WhisperJob>,
	window_tx: std::sync::mpsc::SyncSender<WhisperJob>,
	window_state: WindowState,
	segment_state: SegmentState,
	last_text: String,
	samples_per_read: usize,
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

		for (snapshot, audio_16k) in
			self.window_state.drain_ready_jobs(self.engine_generation, !self.last_text.is_empty())
		{
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

		let has_voice = self.segment_state.peak_mean_abs() >= self.stt_settings.window.min_mean_abs;
		if !has_voice {
			return;
		}

		self.last_text = text.to_string();
		let _ = self.update_tx.blocking_send(AsrUpdate::SherpaPartial(self.last_text.clone()));
	}

	fn handle_endpoint(&mut self, sherpa_text: &str) -> Result<(), AppError> {
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
		let _ = self.second_pass_tx.send(WhisperJob::SecondPass {
			segment_id,
			sample_rate_hz: self.sample_rate,
			samples: segment_samples,
			peak_mean_abs,
		});

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

		let final_text = result.text.trim().to_string();
		let fallback_text = if final_text.is_empty() { self.last_text.clone() } else { final_text };

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
		let _ = self.second_pass_tx.send(WhisperJob::SecondPass {
			segment_id,
			sample_rate_hz: self.sample_rate,
			samples: segment_samples,
			peak_mean_abs,
		});

		Ok(())
	}
}

pub(crate) fn spawn_whisper_worker(
	handle: &tokio::runtime::Handle,
	cancel: CancellationToken,
	whisper_config: stt::WhisperConfig,
	whisper_ctx: Arc<whisper_rs::WhisperContext>,
	stt_settings: SttSettings,
	second_pass_profile: stt::WhisperDecodeProfile,
	window_profile: stt::WhisperDecodeProfile,
	update_tx: mpsc::Sender<AsrUpdate>,
	second_pass_rx: std::sync::mpsc::Receiver<WhisperJob>,
	window_rx: std::sync::mpsc::Receiver<WhisperJob>,
) -> tokio::task::JoinHandle<()> {
	let worker = WhisperWorker {
		cancel,
		whisper_config,
		whisper_ctx,
		stt_settings,
		second_pass_profile,
		window_profile,
		update_tx,
		second_pass_rx,
		window_rx,
	};

	handle.spawn_blocking(move || worker.run())
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
	second_pass_tx: std::sync::mpsc::Sender<WhisperJob>,
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
			second_pass_tx,
			window_tx,
			window_state,
			segment_state,
			last_text: String::new(),
			samples_per_read,
		};

		worker.run()
	})
}
