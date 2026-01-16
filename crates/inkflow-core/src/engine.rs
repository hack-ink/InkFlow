use std::{
	collections::VecDeque,
	sync::{Arc, Mutex},
	time::{Duration, Instant},
};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{
	domain,
	error::AppError,
	settings::SttSettings,
	stt,
};

#[derive(Debug)]
pub enum AsrUpdate {
	SherpaPartial(String),
	WindowScheduled(stt::WindowJobSnapshot),
	WindowResult {
		snapshot: stt::WindowJobSnapshot,
		result: stt::WhisperDecodeResult,
	},
	SegmentEnd {
		segment_id: u64,
		sherpa_text: String,
		committed_end_16k_samples: u64,
		window_generation_after: u64,
	},
	EndpointReset {
		window_generation_after: u64,
	},
	SecondPass {
		segment_id: u64,
		text: String,
	},
}

#[derive(Debug)]
enum WhisperJob {
	SecondPass {
		segment_id: u64,
		sample_rate_hz: u32,
		samples: Vec<f32>,
		peak_mean_abs: f32,
	},
	Window {
		snapshot: stt::WindowJobSnapshot,
		audio_16k: Vec<f32>,
	},
}

pub struct InkFlowEngine {
	runtime: tokio::runtime::Runtime,
	cancel: CancellationToken,
	audio_tx: mpsc::Sender<Vec<f32>>,
	audio_rx: Mutex<Option<mpsc::Receiver<Vec<f32>>>>,
	update_tx: mpsc::Sender<AsrUpdate>,
	update_rx: Mutex<mpsc::Receiver<AsrUpdate>>,
	second_pass_tx: std::sync::mpsc::Sender<WhisperJob>,
	window_tx: std::sync::mpsc::SyncSender<WhisperJob>,
	asr_handle: Mutex<Option<tokio::task::JoinHandle<Result<(), AppError>>>>,
	whisper_handle: tokio::task::JoinHandle<()>,
	stt_settings: SttSettings,
	recognizer: sherpa_onnx::OnlineRecognizer,
	engine_generation: u64,
	sample_rate_hz: Mutex<Option<u32>>,
}

impl InkFlowEngine {
	pub fn start(stt_settings: SttSettings) -> Result<Self, AppError> {
		stt_settings.validate()?;

		let sherpa_config = stt::resolve_sherpa_config(&stt_settings.sherpa)?;
		let recognizer = sherpa_onnx::OnlineRecognizer::new(sherpa_config).map_err(|err| {
			AppError::new(
				"stt_init_failed",
				format!("Failed to initialize sherpa-onnx STT: {err}."),
			)
		})?;

		let whisper_ctx = stt::load_whisper_context(&stt_settings.whisper)?;
		let whisper_config = stt::resolve_whisper_config(&stt_settings.whisper)?;
		let whisper_ctx = Arc::new(whisper_ctx);

		let window_profile =
			stt::WhisperDecodeProfile { best_of: stt_settings.profiles.window_best_of.max(1) };
		let second_pass_profile = stt::WhisperDecodeProfile {
			best_of: stt_settings.profiles.second_pass_best_of.max(1),
		};

		let runtime = tokio::runtime::Builder::new_multi_thread()
			.enable_time()
			.build()
			.map_err(|err| AppError::new("runtime_init_failed", format!("{err}.")))?;

		const AUDIO_QUEUE_CAPACITY: usize = 64;
		const UPDATE_QUEUE_CAPACITY: usize = 64;
		const WINDOW_QUEUE_CAPACITY: usize = 2;

		let (audio_tx, audio_rx) = mpsc::channel::<Vec<f32>>(AUDIO_QUEUE_CAPACITY);
		let (update_tx, update_rx) = mpsc::channel::<AsrUpdate>(UPDATE_QUEUE_CAPACITY);
		let (second_pass_tx, second_pass_rx) = std::sync::mpsc::channel::<WhisperJob>();
		let (window_tx, window_rx) =
			std::sync::mpsc::sync_channel::<WhisperJob>(WINDOW_QUEUE_CAPACITY);

		let cancel = CancellationToken::new();
		let engine_generation = 1;

		let whisper_handle = spawn_whisper_worker(
			runtime.handle(),
			cancel.clone(),
			whisper_config,
			whisper_ctx.clone(),
			stt_settings.clone(),
			second_pass_profile,
			window_profile,
			update_tx.clone(),
			second_pass_rx,
			window_rx,
		);

		Ok(Self {
			runtime,
			cancel,
			audio_tx,
			audio_rx: Mutex::new(Some(audio_rx)),
			update_tx,
			update_rx: Mutex::new(update_rx),
			second_pass_tx,
			window_tx,
			asr_handle: Mutex::new(None),
			whisper_handle,
			stt_settings,
			recognizer,
			engine_generation,
			sample_rate_hz: Mutex::new(None),
		})
	}

	pub fn submit_audio(&self, samples: &[f32], sample_rate_hz: u32) -> Result<(), AppError> {
		if samples.is_empty() {
			return Ok(());
		}
		if sample_rate_hz == 0 {
			return Err(AppError::new(
				"audio_invalid",
				"Sample rate must be greater than zero.",
			));
		}

		self.ensure_asr_worker(sample_rate_hz)?;

		self.audio_tx
			.blocking_send(samples.to_vec())
			.map_err(|_| AppError::new("audio_send_failed", "Failed to submit audio buffer."))
	}

	pub fn poll_update(&self) -> Result<Option<AsrUpdate>, AppError> {
		let mut guard = self.update_rx.lock().map_err(|_| {
			AppError::new("update_receive_failed", "Update receiver is unavailable.")
		})?;

		match guard.try_recv() {
			Ok(update) => Ok(Some(update)),
			Err(tokio::sync::mpsc::error::TryRecvError::Empty) => Ok(None),
			Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
				Err(AppError::new("update_receive_failed", "Update channel disconnected."))
			},
		}
	}

	pub fn stop(self) -> Result<(), AppError> {
		self.cancel.cancel();
		drop(self.audio_tx);

		if let Ok(mut handle) = self.asr_handle.lock() {
			if let Some(handle) = handle.take() {
				let result = self.runtime.block_on(async { handle.await });
				match result {
					Ok(Ok(())) => {},
					Ok(Err(err)) => return Err(err),
					Err(err) => {
						return Err(AppError::new(
							"stt_task_failed",
							format!("The STT task failed: {err}."),
						));
					},
				}
			}
		}

		let result = self.runtime.block_on(async { self.whisper_handle.await });
		if let Err(err) = result {
			return Err(AppError::new(
				"whisper_task_failed",
				format!("The whisper worker task failed: {err}."),
			));
		}

		Ok(())
	}

	fn ensure_asr_worker(&self, sample_rate_hz: u32) -> Result<(), AppError> {
		{
			let mut rate_guard = self.sample_rate_hz.lock().map_err(|_| {
				AppError::new("audio_invalid", "Sample rate state is unavailable.")
			})?;
			match *rate_guard {
				Some(existing) if existing != sample_rate_hz => {
					return Err(AppError::new(
						"audio_invalid",
						"Sample rate must remain constant for the session.",
					));
				},
				Some(_) => {},
				None => {
					*rate_guard = Some(sample_rate_hz);
				},
			}
		}

		let mut handle_guard = self.asr_handle.lock().map_err(|_| {
			AppError::new("stt_task_failed", "ASR task state is unavailable.")
		})?;
		if handle_guard.is_some() {
			return Ok(());
		}

		let audio_rx = {
			let mut guard = self.audio_rx.lock().map_err(|_| {
				AppError::new("audio_invalid", "Audio receiver is unavailable.")
			})?;
			guard.take().ok_or_else(|| {
				AppError::new("audio_invalid", "Audio receiver has already been consumed.")
			})?
		};

		let recognizer = self.recognizer.clone();
		let stream = recognizer.create_stream().map_err(|err| {
			AppError::new(
				"stt_stream_init_failed",
				format!("Failed to create the STT stream: {err}."),
			)
		})?;

		let handle = spawn_asr_worker(
			self.runtime.handle(),
			self.cancel.clone(),
			self.stt_settings.clone(),
			recognizer,
			stream,
			sample_rate_hz,
			self.engine_generation,
			audio_rx,
			self.update_tx.clone(),
			self.second_pass_tx.clone(),
			self.window_tx.clone(),
		);

		*handle_guard = Some(handle);
		Ok(())
	}
}

fn spawn_whisper_worker(
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
	handle.spawn_blocking(move || {
		let min_mean_abs = stt_settings.window.min_mean_abs;
		let window_activity_ms = stt_settings.window.step_ms.clamp(80, 600);
		let window_activity_samples_16k = domain::ms_to_samples_16k(window_activity_ms) as usize;

		loop {
			if cancel.is_cancelled() {
				return;
			}

			while let Ok(job) = second_pass_rx.try_recv() {
				if cancel.is_cancelled() {
					return;
				}

				match job {
					WhisperJob::SecondPass { segment_id, sample_rate_hz, samples, peak_mean_abs } => {
						if samples.is_empty() {
							continue;
						}

						if peak_mean_abs < min_mean_abs {
							continue;
						}

						let audio_16k = stt::resample_linear_to_16k(&samples, sample_rate_hz);
						match stt::transcribe(
							whisper_ctx.as_ref(),
							&audio_16k,
							&whisper_config,
							second_pass_profile,
						) {
							Ok(text) => {
								let text = text.trim().to_string();
								if text.is_empty() {
									continue;
								}
								let _ = update_tx.blocking_send(AsrUpdate::SecondPass { segment_id, text });
							},
							Err(err) => {
								eprintln!(
									"Whisper second-pass transcription failed for segment {}: {}.",
									segment_id, err.message
								);
							},
						}
					},
					WhisperJob::Window { .. } => {},
				}
			}

			match window_rx.recv_timeout(Duration::from_millis(20)) {
				Ok(WhisperJob::Window { snapshot, audio_16k }) => {
					if cancel.is_cancelled() {
						return;
					}

					if audio_16k.is_empty() {
						continue;
					}

					let start =
						audio_16k.len().saturating_sub(window_activity_samples_16k.max(1));
					let mean_abs = mean_abs(&audio_16k[start..]);
					if mean_abs < min_mean_abs {
						continue;
					}

					match stt::transcribe_segments(
						whisper_ctx.as_ref(),
						&audio_16k,
						&whisper_config,
						window_profile,
					) {
						Ok(result) => {
							let _ =
								update_tx.blocking_send(AsrUpdate::WindowResult { snapshot, result });
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
	})
}

#[allow(clippy::too_many_arguments)]
fn spawn_asr_worker(
	handle: &tokio::runtime::Handle,
	cancel: CancellationToken,
	stt_settings: SttSettings,
	recognizer: sherpa_onnx::OnlineRecognizer,
	stream: sherpa_onnx::OnlineStream,
	sample_rate: u32,
	engine_generation: u64,
	mut audio_rx: mpsc::Receiver<Vec<f32>>,
	update_tx: mpsc::Sender<AsrUpdate>,
	second_pass_tx: std::sync::mpsc::Sender<WhisperJob>,
	window_tx: std::sync::mpsc::SyncSender<WhisperJob>,
) -> tokio::task::JoinHandle<Result<(), AppError>> {
	handle.spawn_blocking(move || -> Result<(), AppError> {
		const TAIL_PADDING_MS: u64 = 300;
		const SHERPA_SAMPLE_RATE_HZ: u32 = 16_000;

		let chunk_ms = stt_settings.sherpa.chunk_ms as u64;
		let samples_per_read =
			(sample_rate as u64).saturating_mul(chunk_ms).saturating_div(1_000).max(1) as usize;

		let window_enabled = stt_settings.window.enabled;
		let step = Duration::from_millis(stt_settings.window.step_ms);
		let emit_every = stt_settings.window.emit_every.max(1) as u64;
		let window_len_16k_samples = domain::ms_to_samples_16k(
			stt_settings.window.window_ms.saturating_add(stt_settings.window.context_ms),
		)
		.max(1) as usize;
		let context_len_16k_samples =
			domain::ms_to_samples_16k(stt_settings.window.context_ms) as usize;

		let mut last_text = String::new();
		let mut pending: Vec<f32> = Vec::new();
		let mut pending_start = 0usize;
		let mut segment_id: u64 = 0;
		let mut segment_buffer: Vec<f32> = Vec::new();
		let mut segment_peak_mean_abs: f32 = 0.0;

		let mut window_ring: VecDeque<f32> = VecDeque::with_capacity(window_len_16k_samples);
		let mut total_16k_samples: u64 = 0;
		let mut window_generation: u64 = 0;
		let mut window_job_id: u64 = 0;
		let mut tick_index: u64 = 0;
		let mut next_tick = Instant::now() + step;

		let mut process_samples = |samples: &[f32]| -> Result<(), AppError> {
			if cancel.is_cancelled() {
				return Ok(());
			}

			if samples.is_empty() {
				return Ok(());
			}

			let samples_16k;
			let samples_for_sherpa: &[f32] = if sample_rate == SHERPA_SAMPLE_RATE_HZ {
				samples
			} else {
				samples_16k = stt::resample_linear_to_16k(samples, sample_rate);
				samples_16k.as_slice()
			};

			stream.accept_waveform(SHERPA_SAMPLE_RATE_HZ as i32, samples_for_sherpa);
			segment_buffer.extend_from_slice(samples);
			segment_peak_mean_abs = segment_peak_mean_abs.max(mean_abs(samples));

			if window_enabled {
				if sample_rate == SHERPA_SAMPLE_RATE_HZ {
					total_16k_samples = total_16k_samples.saturating_add(samples.len() as u64);
					window_ring.extend(samples.iter().copied());
				} else {
					total_16k_samples =
						total_16k_samples.saturating_add(samples_for_sherpa.len() as u64);
					window_ring.extend(samples_for_sherpa.iter().copied());
				}
				while window_ring.len() > window_len_16k_samples {
					window_ring.pop_front();
				}
			}

			recognizer.decode(&stream);

			let result = recognizer.result_json(&stream).map_err(|err| {
				AppError::new(
					"stt_decode_failed",
					format!("Failed to decode audio with sherpa-onnx: {err}."),
				)
			})?;

			let text = result.text.trim().to_string();
			if !text.is_empty() && text != last_text {
				let has_voice = segment_peak_mean_abs >= stt_settings.window.min_mean_abs;
				if has_voice {
					last_text = text.clone();
					let _ = update_tx.blocking_send(AsrUpdate::SherpaPartial(text.clone()));
				}
			}

			if window_enabled && !window_ring.is_empty() && !last_text.is_empty() {
				let now = Instant::now();
				while now >= next_tick {
					tick_index = tick_index.saturating_add(1);

					if tick_index.is_multiple_of(emit_every) {
						window_job_id = window_job_id.saturating_add(1);

						let audio_16k: Vec<f32> = window_ring.iter().copied().collect();
						let snapshot = stt::WindowJobSnapshot {
							engine_generation,
							window_generation,
							job_id: window_job_id,
							window_end_16k_samples: total_16k_samples,
							window_len_16k_samples: audio_16k.len(),
							context_len_16k_samples: context_len_16k_samples.min(audio_16k.len()),
						};

						if window_tx
							.try_send(WhisperJob::Window { snapshot: snapshot.clone(), audio_16k })
							.is_ok()
						{
							let _ = update_tx.blocking_send(AsrUpdate::WindowScheduled(snapshot));
						}
					}

					next_tick += step;
				}
			}

			if stream.is_endpoint() {
				let sherpa_text = if text.is_empty() { last_text.clone() } else { text.clone() };
				let has_voice = segment_peak_mean_abs >= stt_settings.window.min_mean_abs;

				window_generation = window_generation.saturating_add(1);

				if !has_voice || sherpa_text.trim().is_empty() {
					let _ = update_tx.blocking_send(AsrUpdate::EndpointReset {
						window_generation_after: window_generation,
					});
					segment_buffer.clear();
					segment_peak_mean_abs = 0.0;
					last_text.clear();
					stream.reset();
					return Ok(());
				}

				segment_id = segment_id.saturating_add(1);
				let _ = update_tx.blocking_send(AsrUpdate::SegmentEnd {
					segment_id,
					sherpa_text,
					committed_end_16k_samples: total_16k_samples,
					window_generation_after: window_generation,
				});

				let segment_samples = std::mem::take(&mut segment_buffer);
				let _ = second_pass_tx.send(WhisperJob::SecondPass {
					segment_id,
					sample_rate_hz: sample_rate,
					samples: segment_samples,
					peak_mean_abs: segment_peak_mean_abs,
				});
				segment_peak_mean_abs = 0.0;

				last_text.clear();
				stream.reset();
			}

			Ok(())
		};

		while let Some(chunk) = audio_rx.blocking_recv() {
			if cancel.is_cancelled() {
				return Ok(());
			}

			pending.extend_from_slice(&chunk);

			while pending.len().saturating_sub(pending_start) >= samples_per_read {
				let end = pending_start.saturating_add(samples_per_read);
				process_samples(&pending[pending_start..end])?;
				pending_start = end;

				if pending_start >= 8_192 && pending_start >= pending.len().saturating_div(2) {
					pending.drain(..pending_start);
					pending_start = 0;
				}
			}
		}

		if pending_start < pending.len() {
			process_samples(&pending[pending_start..])?;
		}

		if cancel.is_cancelled() {
			return Ok(());
		}

		let tail_samples =
			(sample_rate as u64).saturating_mul(TAIL_PADDING_MS).saturating_div(1_000) as usize;
		if tail_samples > 0 {
			let tail = vec![0.0f32; tail_samples];
			let tail_16k_samples = (SHERPA_SAMPLE_RATE_HZ as u64)
				.saturating_mul(TAIL_PADDING_MS)
				.saturating_div(1_000) as usize;
			if tail_16k_samples > 0 {
				let tail_16k = vec![0.0f32; tail_16k_samples];
				stream.accept_waveform(SHERPA_SAMPLE_RATE_HZ as i32, &tail_16k);
			}
			segment_buffer.extend_from_slice(&tail);
		}

		stream.input_finished();
		recognizer.decode(&stream);

		let result = recognizer.result_json(&stream).map_err(|err| {
			AppError::new(
				"stt_decode_failed",
				format!("Failed to decode audio with sherpa-onnx: {err}."),
			)
		})?;

		let final_text = result.text.trim().to_string();
		let fallback_text = if final_text.is_empty() { last_text } else { final_text };

		if segment_buffer.is_empty() {
			return Ok(());
		}

		let has_voice = segment_peak_mean_abs >= stt_settings.window.min_mean_abs;

		window_generation = window_generation.saturating_add(1);

		if !has_voice || fallback_text.trim().is_empty() {
			let _ = update_tx.blocking_send(AsrUpdate::EndpointReset {
				window_generation_after: window_generation,
			});
			return Ok(());
		}

		segment_id = segment_id.saturating_add(1);
		let _ = update_tx.blocking_send(AsrUpdate::SegmentEnd {
			segment_id,
			sherpa_text: fallback_text,
			committed_end_16k_samples: total_16k_samples,
			window_generation_after: window_generation,
		});

		let segment_samples = std::mem::take(&mut segment_buffer);
		let _ = second_pass_tx.send(WhisperJob::SecondPass {
			segment_id,
			sample_rate_hz: sample_rate,
			samples: segment_samples,
			peak_mean_abs: segment_peak_mean_abs,
		});

		Ok(())
	})
}

fn mean_abs(samples: &[f32]) -> f32 {
	if samples.is_empty() {
		return 0.0;
	}

	let mut sum = 0.0_f32;
	for s in samples {
		sum += s.abs();
	}

	sum / samples.len() as f32
}
