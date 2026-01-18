mod modes;
mod state;

use std::{
	sync::{Arc, Mutex},
	time::Duration,
};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{domain, error::AppError, settings::SttSettings, stt};

use modes::{DecodeMode, InferenceMode, ModeRouter, PipelinePlan};
use state::{SegmentState, WindowState};

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
	SecondPass { segment_id: u64, sample_rate_hz: u32, samples: Vec<f32>, peak_mean_abs: f32 },
	Window { snapshot: stt::WindowJobSnapshot, audio_16k: Vec<f32> },
}

pub struct InkFlowEngine {
	pipeline: SttPipeline,
}

enum SttPipeline {
	LocalStreamSecondPass(LocalStreamSecondPassPipeline),
}

impl InkFlowEngine {
	pub fn start(stt_settings: SttSettings) -> Result<Self, AppError> {
		let plan = ModeRouter::resolve(&stt_settings);
		let pipeline = SttPipeline::start(plan, stt_settings)?;
		Ok(Self { pipeline })
	}

	pub fn submit_audio(&self, samples: &[f32], sample_rate_hz: u32) -> Result<(), AppError> {
		self.pipeline.submit_audio(samples, sample_rate_hz)
	}

	pub fn poll_update(&self) -> Result<Option<AsrUpdate>, AppError> {
		self.pipeline.poll_update()
	}

	pub fn stop(self) -> Result<(), AppError> {
		self.pipeline.stop()
	}
}

impl SttPipeline {
	fn start(plan: PipelinePlan, stt_settings: SttSettings) -> Result<Self, AppError> {
		match (plan.decode_mode, plan.inference) {
			(DecodeMode::StreamSecondPass, InferenceMode::LocalOnly) =>
				Ok(SttPipeline::LocalStreamSecondPass(LocalStreamSecondPassPipeline::start(
					stt_settings,
					plan.window_enabled,
				)?)),
		}
	}

	fn submit_audio(&self, samples: &[f32], sample_rate_hz: u32) -> Result<(), AppError> {
		match self {
			SttPipeline::LocalStreamSecondPass(pipeline) =>
				pipeline.submit_audio(samples, sample_rate_hz),
		}
	}

	fn poll_update(&self) -> Result<Option<AsrUpdate>, AppError> {
		match self {
			SttPipeline::LocalStreamSecondPass(pipeline) => pipeline.poll_update(),
		}
	}

	fn stop(self) -> Result<(), AppError> {
		match self {
			SttPipeline::LocalStreamSecondPass(pipeline) => pipeline.stop(),
		}
	}
}

struct LocalStreamSecondPassPipeline {
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
	window_enabled: bool,
}

impl LocalStreamSecondPassPipeline {
	fn start(stt_settings: SttSettings, window_enabled: bool) -> Result<Self, AppError> {
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
		let second_pass_profile =
			stt::WhisperDecodeProfile { best_of: stt_settings.profiles.second_pass_best_of.max(1) };

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
			whisper_ctx,
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
			window_enabled,
		})
	}

	fn submit_audio(&self, samples: &[f32], sample_rate_hz: u32) -> Result<(), AppError> {
		if samples.is_empty() {
			return Ok(());
		}
		if sample_rate_hz == 0 {
			return Err(AppError::new("audio_invalid", "Sample rate must be greater than zero."));
		}

		self.ensure_asr_worker(sample_rate_hz)?;

		self.audio_tx
			.blocking_send(samples.to_vec())
			.map_err(|_| AppError::new("audio_send_failed", "Failed to submit audio buffer."))
	}

	fn poll_update(&self) -> Result<Option<AsrUpdate>, AppError> {
		let mut guard = self.update_rx.lock().map_err(|_| {
			AppError::new("update_receive_failed", "Update receiver is unavailable.")
		})?;

		match guard.try_recv() {
			Ok(update) => Ok(Some(update)),
			Err(tokio::sync::mpsc::error::TryRecvError::Empty) => Ok(None),
			Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) =>
				Err(AppError::new("update_receive_failed", "Update channel disconnected.")),
		}
	}

	fn stop(self) -> Result<(), AppError> {
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
			let mut rate_guard = self
				.sample_rate_hz
				.lock()
				.map_err(|_| AppError::new("audio_invalid", "Sample rate state is unavailable."))?;
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

		let mut handle_guard = self
			.asr_handle
			.lock()
			.map_err(|_| AppError::new("stt_task_failed", "ASR task state is unavailable."))?;
		if handle_guard.is_some() {
			return Ok(());
		}

		let audio_rx = {
			let mut guard = self
				.audio_rx
				.lock()
				.map_err(|_| AppError::new("audio_invalid", "Audio receiver is unavailable."))?;
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
			self.window_enabled,
		);

		*handle_guard = Some(handle);
		Ok(())
	}
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
					let mean_abs = mean_abs(&audio_16k[start..]);
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

#[allow(clippy::too_many_arguments)]
fn spawn_asr_worker(
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
