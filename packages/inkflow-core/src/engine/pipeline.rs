use std::{
	collections::VecDeque,
	sync::{Arc, Mutex},
};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{error::AppError, settings::SttSettings, stt};

use super::{
	AsrUpdate,
	modes::{DecodeMode, InferenceMode, PipelinePlan},
	queue::SecondPassQueue,
	render::RenderState,
	worker::{SpeechActivity, StreamCommand, WhisperJob, spawn_asr_worker, spawn_whisper_worker},
};

pub(crate) enum SttPipeline {
	LocalStreamSecondPass(LocalStreamSecondPassPipeline),
}

impl SttPipeline {
	pub(crate) fn start(plan: PipelinePlan, stt_settings: SttSettings) -> Result<Self, AppError> {
		match (plan.decode_mode, plan.inference) {
			(DecodeMode::StreamSecondPass, InferenceMode::LocalOnly) =>
				Ok(SttPipeline::LocalStreamSecondPass(LocalStreamSecondPassPipeline::start(
					stt_settings,
					plan.window_enabled,
				)?)),
		}
	}

	pub(crate) fn submit_audio(
		&self,
		samples: &[f32],
		sample_rate_hz: u32,
	) -> Result<(), AppError> {
		match self {
			SttPipeline::LocalStreamSecondPass(pipeline) =>
				pipeline.submit_audio(samples, sample_rate_hz),
		}
	}

	pub(crate) fn force_finalize(&self) -> Result<(), AppError> {
		match self {
			SttPipeline::LocalStreamSecondPass(pipeline) => pipeline.force_finalize(),
		}
	}

	pub(crate) fn poll_update(&self) -> Result<Option<AsrUpdate>, AppError> {
		match self {
			SttPipeline::LocalStreamSecondPass(pipeline) => pipeline.poll_update(),
		}
	}

	pub(crate) fn stop(self) -> Result<(), AppError> {
		match self {
			SttPipeline::LocalStreamSecondPass(pipeline) => pipeline.stop(),
		}
	}
}

pub(crate) struct LocalStreamSecondPassPipeline {
	runtime: tokio::runtime::Runtime,
	cancel: CancellationToken,
	audio_tx: mpsc::Sender<StreamCommand>,
	audio_rx: Mutex<Option<mpsc::Receiver<StreamCommand>>>,
	raw_update_tx: mpsc::Sender<AsrUpdate>,
	raw_update_rx: Mutex<mpsc::Receiver<AsrUpdate>>,
	second_pass_queue: Arc<SecondPassQueue>,
	speech_activity: Arc<SpeechActivity>,
	window_tx: std::sync::mpsc::SyncSender<WhisperJob>,
	asr_handle: Mutex<Option<tokio::task::JoinHandle<Result<(), AppError>>>>,
	whisper_handle: tokio::task::JoinHandle<()>,
	stt_settings: SttSettings,
	recognizer: sherpa_onnx::OnlineRecognizer,
	engine_generation: u64,
	sample_rate_hz: Mutex<Option<u32>>,
	window_enabled: bool,
	render_state: Mutex<RenderState>,
	pending_updates: Mutex<VecDeque<AsrUpdate>>,
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

		let (audio_tx, audio_rx) = mpsc::channel::<StreamCommand>(AUDIO_QUEUE_CAPACITY);
		let (raw_update_tx, raw_update_rx) = mpsc::channel::<AsrUpdate>(UPDATE_QUEUE_CAPACITY);
		let second_pass_queue =
			Arc::new(SecondPassQueue::new(stt_settings.second_pass_queue_capacity));
		let (window_tx, window_rx) =
			std::sync::mpsc::sync_channel::<WhisperJob>(WINDOW_QUEUE_CAPACITY);

		let cancel = CancellationToken::new();
		let engine_generation = 1;
		let speech_activity = Arc::new(SpeechActivity::new());

		let whisper_handle = spawn_whisper_worker(
			runtime.handle(),
			cancel.clone(),
			whisper_config,
			whisper_ctx,
			stt_settings.clone(),
			second_pass_profile,
			window_profile,
			raw_update_tx.clone(),
			second_pass_queue.clone(),
			speech_activity.clone(),
			window_rx,
		);

		Ok(Self {
			runtime,
			cancel,
			audio_tx,
			audio_rx: Mutex::new(Some(audio_rx)),
			raw_update_tx,
			raw_update_rx: Mutex::new(raw_update_rx),
			second_pass_queue,
			speech_activity,
			window_tx,
			asr_handle: Mutex::new(None),
			whisper_handle,
			stt_settings,
			recognizer,
			engine_generation,
			sample_rate_hz: Mutex::new(None),
			window_enabled,
			render_state: Mutex::new(RenderState::new()),
			pending_updates: Mutex::new(VecDeque::new()),
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
			.blocking_send(StreamCommand::Audio(samples.to_vec()))
			.map_err(|_| AppError::new("audio_send_failed", "Failed to submit audio buffer."))
	}

	fn force_finalize(&self) -> Result<(), AppError> {
		let has_worker = {
			let guard = self
				.asr_handle
				.lock()
				.map_err(|_| AppError::new("stt_task_failed", "ASR task state is unavailable."))?;
			guard.is_some()
		};

		if !has_worker {
			let _ = self.raw_update_tx.blocking_send(AsrUpdate::EndpointReset {
				window_generation_after: 1,
			});
			return Ok(());
		}

		self.audio_tx
			.blocking_send(StreamCommand::Finalize)
			.map_err(|_| AppError::new("audio_send_failed", "Failed to submit finalize signal."))
	}

	fn poll_update(&self) -> Result<Option<AsrUpdate>, AppError> {
		let mut pending = self.pending_updates.lock().map_err(|_| {
			AppError::new("update_receive_failed", "Pending update queue is unavailable.")
		})?;
		if let Some(next) = pending.pop_front() {
			return Ok(Some(next));
		}

		let mut guard = self.raw_update_rx.lock().map_err(|_| {
			AppError::new("update_receive_failed", "Update receiver is unavailable.")
		})?;

		loop {
			match guard.try_recv() {
				Ok(update) => {
					if let Ok(mut renderer) = self.render_state.lock() {
						if let Some(rendered) = renderer.handle_update(&update, &self.stt_settings)
						{
							pending.push_back(rendered);
						}

						if renderer.should_forward(&update) {
							pending.push_back(update);
						}
					} else {
						pending.push_back(update);
					}
					if let Some(next) = pending.pop_front() {
						return Ok(Some(next));
					}
				},
				Err(tokio::sync::mpsc::error::TryRecvError::Empty) => return Ok(None),
				Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
					return Err(AppError::new(
						"update_receive_failed",
						"Update channel disconnected.",
					));
				},
			}
		}
	}

	fn stop(self) -> Result<(), AppError> {
		self.cancel.cancel();
		drop(self.audio_tx);

		if let Ok(mut handle) = self.asr_handle.lock()
			&& let Some(handle) = handle.take() {
				let result = self.runtime.block_on(handle);
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

		let result = self.runtime.block_on(self.whisper_handle);
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
			self.raw_update_tx.clone(),
			self.second_pass_queue.clone(),
			self.speech_activity.clone(),
			self.window_tx.clone(),
			self.window_enabled,
		);

		*handle_guard = Some(handle);
		Ok(())
	}
}
