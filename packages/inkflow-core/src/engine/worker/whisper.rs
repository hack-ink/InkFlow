use std::{sync::Arc, time::Duration};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{domain, settings::SttSettings, stt};

use super::audio::{ActivityGate, SpeechActivity, WhisperJob, activity_metrics, audio_hash};
use crate::engine::{AsrUpdate, queue::SecondPassQueue};

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
	window_gate_blocked: bool,
	speech_activity: Arc<SpeechActivity>,
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

			self.drain_second_pass();

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
					if !self.speech_activity.is_recent(window_activity_ms) && !gate.allows(&metrics)
					{
						if !self.window_gate_blocked {
							self.window_gate_blocked = true;
							tracing::debug!(
								mean_abs = metrics.mean_abs,
								rms = metrics.rms,
								zero_crossing_rate = metrics.zero_crossing_rate,
								band_energy_ratio = metrics.band_energy_ratio,
								min_mean_abs = gate.min_mean_abs,
								min_rms = gate.min_rms,
								max_zero_crossing_rate = gate.max_zero_crossing_rate,
								min_band_energy_ratio = gate.min_band_energy_ratio,
								"Window activity gate suppressed decoding."
							);
						}
						continue;
					}
					if self.window_gate_blocked {
						self.window_gate_blocked = false;
						tracing::debug!("Window activity gate resumed decoding.");
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
							tracing::error!(
								error = %err.message,
								"Whisper window transcription failed."
							);
						},
					}
				},
				Ok(WhisperJob::SecondPass { .. }) => {},
				Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {},
				Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
			}
		}
	}

	fn drain_second_pass(&mut self) {
		loop {
			let Some(job) = self.second_pass_queue.pop(Duration::from_millis(1)) else {
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

			tracing::debug!(segment_id, samples = samples.len(), "Second-pass dequeued.");

			let audio_16k = stt::resample_linear_to_16k(&samples, sample_rate_hz);
			match stt::transcribe_segments(
				self.whisper_ctx.as_ref(),
				&audio_16k,
				&self.whisper_config,
				self.second_pass_profile,
			) {
				Ok(result) => {
					let text = result.text.trim().to_string();
					if text.is_empty() {
						tracing::debug!(segment_id, "Second-pass returned empty text.");
						continue;
					}
					let _ =
						self.update_tx.blocking_send(AsrUpdate::SecondPass { segment_id, text });
					tracing::info!(
						segment_id,
						peak_mean_abs,
						"Second-pass transcription delivered."
					);
				},
				Err(err) => {
					tracing::error!(
						segment_id,
						error = %err.message,
						"Whisper second-pass transcription failed."
					);
				},
			}
		}
	}
}

struct WindowDecodeCache {
	hash: u64,
	len: usize,
	result: stt::WhisperDecodeResult,
}

#[allow(clippy::too_many_arguments)]
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
	speech_activity: Arc<SpeechActivity>,
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
		speech_activity,
		window_rx,
		window_cache: None,
		window_gate_blocked: false,
	};

	handle.spawn_blocking(move || worker.run())
}
