use std::{
	collections::hash_map::DefaultHasher,
	hash::{Hash, Hasher},
	sync::Arc,
	time::Duration,
};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{domain, error::AppError, settings::SttSettings, stt};

use super::{
	AsrUpdate,
	queue::SecondPassQueue,
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
	second_pass_queue: Arc<SecondPassQueue>,
	window_rx: std::sync::mpsc::Receiver<WhisperJob>,
	window_cache: Option<WindowDecodeCache>,
}

impl WhisperWorker {
	fn run(mut self) {
		let gate = ActivityGate::new(&self.stt_settings.window);
		let window_activity_ms = self.stt_settings.window.step_ms.clamp(80, 600);
		let window_activity_samples_16k = domain::ms_to_samples_16k(window_activity_ms) as usize;

		loop {
			if self.cancel.is_cancelled() {
				return;
			}

			self.drain_second_pass(&gate);

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
					let metrics = activity_metrics(&audio_16k[start..], 16_000);
					if !gate.allows(&metrics) {
						continue;
					}

					let hash = audio_hash(&audio_16k);
					if let Some(cache) = self.window_cache.as_ref()
						&& cache.hash == hash
						&& cache.len == audio_16k.len()
					{
						let _ = self.update_tx.blocking_send(AsrUpdate::WindowResult {
							snapshot,
							result: cache.result.clone(),
						});
						continue;
					}

					match stt::transcribe_segments(
						self.whisper_ctx.as_ref(),
						&audio_16k,
						&self.whisper_config,
						self.window_profile,
					) {
						Ok(result) => {
							self.window_cache = Some(WindowDecodeCache {
								hash,
								len: audio_16k.len(),
								result: result.clone(),
							});
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

	fn drain_second_pass(&self, gate: &ActivityGate) {
		loop {
			let Some(job) =
				self.second_pass_queue.pop(Duration::from_millis(1))
			else {
				break;
			};

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

			let audio_16k = stt::resample_linear_to_16k(&samples, sample_rate_hz);
			let metrics = activity_metrics(&audio_16k, 16_000);
			if !gate.allows_with_peak(&metrics, peak_mean_abs) {
				continue;
			}
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
	second_pass_queue: Arc<SecondPassQueue>,
	window_tx: std::sync::mpsc::SyncSender<WhisperJob>,
	window_state: WindowState,
	segment_state: SegmentState,
	last_text: String,
	samples_per_read: usize,
	pending_second_pass: Option<PendingSecondPass>,
}

struct PendingSecondPass {
	segment_id: u64,
	samples: Vec<f32>,
	peak_mean_abs: f32,
	remaining_tail_samples: usize,
}

impl PendingSecondPass {
	fn append_tail(&mut self, samples: &[f32]) -> bool {
		if self.remaining_tail_samples == 0 || samples.is_empty() {
			return self.remaining_tail_samples == 0;
		}

		let take = self.remaining_tail_samples.min(samples.len());
		self.samples.extend_from_slice(&samples[..take]);
		self.remaining_tail_samples = self.remaining_tail_samples.saturating_sub(take);
		self.remaining_tail_samples == 0
	}
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

		let allow_windows = self.second_pass_queue.len()
			< self.stt_settings.window.window_backpressure_high_watermark;
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

		let has_voice = self.segment_state.peak_mean_abs() >= self.stt_settings.window.min_mean_abs;
		if !has_voice {
			return;
		}

		self.last_text = text.to_string();
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
		self.schedule_second_pass(segment_id, segment_samples, peak_mean_abs);
		self.flush_pending_second_pass(true);

		Ok(())
	}

	fn append_pending_tail(&mut self, samples: &[f32]) {
		let Some(pending) = self.pending_second_pass.as_mut() else {
			return;
		};

		if pending.append_tail(samples) {
			self.flush_pending_second_pass(true);
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
		if tail_samples == 0 {
			self.enqueue_second_pass(segment_id, segment_samples, peak_mean_abs);
			return;
		}

		self.pending_second_pass = Some(PendingSecondPass {
			segment_id,
			samples: segment_samples,
			peak_mean_abs,
			remaining_tail_samples: tail_samples,
		});
	}

	fn flush_pending_second_pass(&mut self, force: bool) {
		let Some(pending) = self.pending_second_pass.take() else {
			return;
		};

		if pending.remaining_tail_samples == 0 || force {
			self.enqueue_second_pass(
				pending.segment_id,
				pending.samples,
				pending.peak_mean_abs,
			);
			return;
		}

		self.pending_second_pass = Some(pending);
	}

	fn enqueue_second_pass(&self, segment_id: u64, samples: Vec<f32>, peak_mean_abs: f32) {
		let _ = self.second_pass_queue.push(WhisperJob::SecondPass {
			segment_id,
			sample_rate_hz: self.sample_rate,
			samples,
			peak_mean_abs,
		});
	}
}

fn ms_to_samples(sample_rate_hz: u32, ms: u64) -> usize {
	if sample_rate_hz == 0 || ms == 0 {
		return 0;
	}

	(sample_rate_hz as u64)
		.saturating_mul(ms)
		.saturating_div(1_000) as usize
}

struct WindowDecodeCache {
	hash: u64,
	len: usize,
	result: stt::WhisperDecodeResult,
}

fn audio_hash(samples: &[f32]) -> u64 {
	let mut hasher = DefaultHasher::new();
	let len = samples.len();
	let stride = (len / 128).max(1);
	for sample in samples.iter().step_by(stride) {
		sample.to_bits().hash(&mut hasher);
	}
	len.hash(&mut hasher);
	hasher.finish()
}

struct ActivityGate {
	min_mean_abs: f32,
	min_rms: f32,
	max_zero_crossing_rate: f32,
	min_band_energy_ratio: f32,
}

impl ActivityGate {
	fn new(settings: &crate::settings::WhisperWindowSettings) -> Self {
		Self {
			min_mean_abs: settings.min_mean_abs,
			min_rms: settings.min_rms,
			max_zero_crossing_rate: settings.max_zero_crossing_rate,
			min_band_energy_ratio: settings.min_band_energy_ratio,
		}
	}

	fn allows(&self, metrics: &ActivityMetrics) -> bool {
		metrics.mean_abs >= self.min_mean_abs
			&& metrics.rms >= self.min_rms
			&& metrics.zero_crossing_rate <= self.max_zero_crossing_rate
			&& metrics.band_energy_ratio >= self.min_band_energy_ratio
	}

	fn allows_with_peak(&self, metrics: &ActivityMetrics, peak_mean_abs: f32) -> bool {
		peak_mean_abs >= self.min_mean_abs && self.allows(metrics)
	}
}

struct ActivityMetrics {
	mean_abs: f32,
	rms: f32,
	zero_crossing_rate: f32,
	band_energy_ratio: f32,
}

fn activity_metrics(samples: &[f32], sample_rate_hz: u32) -> ActivityMetrics {
	if samples.is_empty() || sample_rate_hz == 0 {
		return ActivityMetrics {
			mean_abs: 0.0,
			rms: 0.0,
			zero_crossing_rate: 0.0,
			band_energy_ratio: 0.0,
		};
	}

	let mut sum_abs = 0.0_f32;
	let mut sum_sq = 0.0_f32;
	let mut zero_crossings = 0usize;
	let mut prev = samples[0];
	for &sample in samples {
		sum_abs += sample.abs();
		sum_sq += sample * sample;
		if (sample >= 0.0 && prev < 0.0) || (sample < 0.0 && prev >= 0.0) {
			zero_crossings += 1;
		}
		prev = sample;
	}

	let len = samples.len() as f32;
	let mean_abs = sum_abs / len;
	let rms = (sum_sq / len).sqrt();
	let denom = samples.len().saturating_sub(1).max(1) as f32;
	let zero_crossing_rate = zero_crossings as f32 / denom;
	let band_energy_ratio = band_energy_ratio(samples, sample_rate_hz);

	ActivityMetrics { mean_abs, rms, zero_crossing_rate, band_energy_ratio }
}

fn band_energy_ratio(samples: &[f32], sample_rate_hz: u32) -> f32 {
	use std::f32::consts::PI;

	if samples.is_empty() || sample_rate_hz == 0 {
		return 0.0;
	}

	let dt = 1.0_f32 / sample_rate_hz as f32;
	let hp_rc = 1.0_f32 / (2.0_f32 * PI * 300.0_f32);
	let lp_rc = 1.0_f32 / (2.0_f32 * PI * 3400.0_f32);
	let hp_alpha = hp_rc / (hp_rc + dt);
	let lp_alpha = dt / (lp_rc + dt);

	let mut hp_out = 0.0_f32;
	let mut hp_prev = 0.0_f32;
	let mut lp_out = 0.0_f32;
	let mut total_energy = 0.0_f32;
	let mut band_energy = 0.0_f32;

	for &sample in samples {
		total_energy += sample * sample;
		hp_out = hp_alpha * (hp_out + sample - hp_prev);
		hp_prev = sample;
		lp_out += lp_alpha * (hp_out - lp_out);
		band_energy += lp_out * lp_out;
	}

	if total_energy <= 0.0 {
		return 0.0;
	}

	(band_energy / total_energy).clamp(0.0, 1.0)
}

#[cfg(test)]
mod activity_tests {
	use super::activity_metrics;

	#[test]
	fn activity_metrics_detects_silence() {
		let samples = vec![0.0_f32; 160];
		let metrics = activity_metrics(&samples, 16_000);
		assert!(metrics.rms <= 1e-6);
		assert!(metrics.mean_abs <= 1e-6);
	}

	#[test]
	fn activity_metrics_zero_crossing_increases_with_alternation() {
		let mut samples = Vec::new();
		for i in 0..200 {
			let value = if i % 2 == 0 { 0.8 } else { -0.8 };
			samples.push(value);
		}
		let metrics = activity_metrics(&samples, 16_000);
		assert!(metrics.zero_crossing_rate > 0.4);
	}
}

#[cfg(test)]
mod pending_tests {
	use super::PendingSecondPass;

	#[test]
	fn pending_second_pass_appends_tail_until_complete() {
		let mut pending = PendingSecondPass {
			segment_id: 1,
			samples: vec![0.1; 4],
			peak_mean_abs: 0.1,
			remaining_tail_samples: 4,
		};

		assert!(!pending.append_tail(&[0.2, 0.2]));
		assert_eq!(pending.remaining_tail_samples, 2);

		assert!(pending.append_tail(&[0.3, 0.3]));
		assert_eq!(pending.remaining_tail_samples, 0);
		assert_eq!(pending.samples.len(), 8);
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
	second_pass_queue: Arc<SecondPassQueue>,
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
		second_pass_queue,
		window_rx,
		window_cache: None,
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
	second_pass_queue: Arc<SecondPassQueue>,
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
			window_tx,
			window_state,
			segment_state,
			last_text: String::new(),
			samples_per_read,
			pending_second_pass: None,
		};

		worker.run()
	})
}
