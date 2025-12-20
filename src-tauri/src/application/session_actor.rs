use std::{
	sync::Arc,
	time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot, watch};
use tokio_util::sync::CancellationToken;

use crate::{
	domain::{
		MergeState, choose_segment_provisional_text, collapse_leading_duplicate_word, dedup_tail,
		extract_window_tail_text, leading_words_compatible, should_accept_second_pass_replacement,
		token_mode_for_language,
	},
	engine::EngineManager,
	error::AppError,
	events::{
		ErrorEvent, LlmRewriteEvent, SessionStateEvent, SttFinalEvent, SttPartialEvent, SttStrategy,
	},
	llm,
	pipeline::{AsrUpdate, DictationHandle, DictationInit, DictationPipeline},
	ports::{PlatformPort, UiPort},
	settings::{SettingsStore, SttSettings},
	stt,
	stt_trace::{SttTrace, TraceDetails, write_recorded_audio_wav},
};

type SttHandle = DictationHandle;

type SessionReply = oneshot::Sender<Result<SessionSnapshot, AppError>>;

#[derive(Debug, Copy, Clone, Serialize, PartialEq, Eq)]
pub enum SessionState {
	Hidden,
	Showing,
	Listening,
	Finalizing,
	Rewriting,
	RewriteReady,
	Injecting,
	Error,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionAction {
	Show,
	StartNew,
	Enter,
	Escape,
	Rewrite,
}

pub(crate) enum SessionCommand {
	AttachContext(Arc<SessionContext>),
	UserAction {
		action: SessionAction,
		reply: SessionReply,
	},
	PipelineUpdate(AsrUpdate),
	FinalizeReady {
		session_id: String,
		result: Result<(), AppError>,
	},
	RewriteDone {
		session_id: String,
		result: Result<llm::RewriteResult, AppError>,
		fallback_text: String,
	},
	InjectDone {
		session_id: String,
		result: Result<(), AppError>,
	},
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionSnapshot {
	pub session_id: Option<String>,
	pub state: SessionState,
	pub raw_text: String,
	pub rewrite_text: Option<String>,
}

#[derive(Clone)]
pub(crate) struct SessionContext {
	app: tauri::AppHandle,
	engine: Arc<EngineManager>,
	settings: Arc<SettingsStore>,
	ui: Arc<dyn UiPort>,
	platform: Arc<dyn PlatformPort>,
}
impl SessionContext {
	pub(crate) fn new(
		app: tauri::AppHandle,
		engine: Arc<EngineManager>,
		settings: Arc<SettingsStore>,
		ui: Arc<dyn UiPort>,
		platform: Arc<dyn PlatformPort>,
	) -> Self {
		Self { app, engine, settings, ui, platform }
	}

	pub(crate) async fn stt_settings(&self) -> Result<SttSettings, AppError> {
		self.settings.stt(&self.app).await
	}

	pub(crate) async fn llm_settings(
		&self,
	) -> Result<Option<crate::settings::LlmSettingsResolved>, AppError> {
		self.settings.llm_resolved(&self.app).await
	}

	pub(crate) async fn get_sherpa(
		&self,
		stt_settings: &SttSettings,
	) -> Result<sherpa_onnx::OnlineRecognizer, AppError> {
		self.engine.get_sherpa(&self.app, stt_settings).await
	}

	pub(crate) async fn get_whisper(
		&self,
		stt_settings: &SttSettings,
	) -> Result<Arc<whisper_rs::WhisperContext>, AppError> {
		self.engine.get_whisper(&self.app, stt_settings).await
	}

	pub(crate) async fn set_engine_error(&self, err: AppError) {
		self.engine.set_error(&self.app, err).await;
	}

	pub(crate) fn engine_generation(&self) -> u64 {
		self.engine.current_engine_generation()
	}

	pub(crate) fn emit_session_state(&self, payload: SessionStateEvent) {
		self.ui.emit_session_state(payload);
	}

	pub(crate) fn emit_stt_partial(&self, payload: SttPartialEvent) {
		self.ui.emit_stt_partial(payload);
	}

	pub(crate) fn emit_stt_final(&self, payload: SttFinalEvent) {
		self.ui.emit_stt_final(payload);
	}

	pub(crate) fn emit_llm_rewrite(&self, payload: LlmRewriteEvent) {
		self.ui.emit_llm_rewrite(payload);
	}

	pub(crate) fn emit_error(&self, payload: ErrorEvent) {
		self.ui.emit_error(payload);
	}

	pub(crate) fn hide_overlay_window(&self) {
		self.ui.hide_overlay_window();
	}

	pub(crate) fn show_overlay_window(&self) {
		self.ui.show_overlay_window();
	}

	pub(crate) fn platform(&self) -> Arc<dyn PlatformPort> {
		self.platform.clone()
	}
}

pub(crate) struct SessionActor {
	command_rx: mpsc::Receiver<SessionCommand>,
	command_tx: mpsc::Sender<SessionCommand>,
	state_tx: watch::Sender<SessionState>,
	context: Option<Arc<SessionContext>>,
	next_session_id: u64,
	session_id: Option<String>,
	state: SessionState,
	raw_text: String,
	stt_segments: Vec<String>,
	stt_live_text: String,
	stt_live_has_window: bool,
	rewrite_text: Option<String>,
	stt_revision: u64,
	committed_end_16k_samples: u64,
	window_generation: u64,
	window_job_id_last_scheduled: u64,
	window_job_id_last_applied: u64,
	merge: MergeState,
	last_activity_at: Instant,
	stt_runtime: Option<SttRuntime>,
	trace: Option<SttTrace>,
	stt_settings: Option<SttSettings>,
}
impl SessionActor {
	pub(crate) fn spawn() -> (mpsc::Sender<SessionCommand>, watch::Receiver<SessionState>) {
		let (command_tx, command_rx) = mpsc::channel(64);
		let (state_tx, state_rx) = watch::channel(SessionState::Hidden);
		let actor = Self {
			command_rx,
			command_tx: command_tx.clone(),
			state_tx,
			context: None,
			next_session_id: 1,
			session_id: None,
			state: SessionState::Hidden,
			raw_text: String::new(),
			stt_segments: Vec::new(),
			stt_live_text: String::new(),
			stt_live_has_window: false,
			rewrite_text: None,
			stt_revision: 0,
			committed_end_16k_samples: 0,
			window_generation: 0,
			window_job_id_last_scheduled: 0,
			window_job_id_last_applied: 0,
			merge: MergeState::default(),
			last_activity_at: Instant::now(),
			stt_runtime: None,
			trace: None,
			stt_settings: None,
		};

		tauri::async_runtime::spawn(actor.run());

		(command_tx, state_rx)
	}

	pub(crate) async fn run(mut self) {
		while let Some(command) = self.command_rx.recv().await {
			match command {
				SessionCommand::AttachContext(context) => {
					self.context = Some(context);
				},
				SessionCommand::UserAction { action, reply } => {
					let result = self.handle_action(action).await;
					let _ = reply.send(result);
				},
				SessionCommand::PipelineUpdate(update) => {
					self.handle_pipeline_update(update).await;
				},
				SessionCommand::FinalizeReady { session_id, result } => {
					self.handle_finalize_ready(&session_id, result).await;
				},
				SessionCommand::RewriteDone { session_id, result, fallback_text } => {
					self.handle_rewrite_done(&session_id, result, fallback_text).await;
				},
				SessionCommand::InjectDone { session_id, result } => {
					self.handle_inject_done(&session_id, result).await;
				},
			}
		}
	}

	async fn handle_action(&mut self, action: SessionAction) -> Result<SessionSnapshot, AppError> {
		match action {
			SessionAction::Show =>
				if self.session_id.is_none() {
					let id = self.next_session_id;
					self.next_session_id = self.next_session_id.saturating_add(1);
					self.session_id = Some(id.to_string());
					self.reset_transcription_state_with_activity();

					self.set_state(
						SessionState::Showing,
						Some("Ready. Hold Space to talk.".to_string()),
					);
				},
			SessionAction::StartNew => {
				let _ = self.cancel_stt(true);

				let id = self.next_session_id;
				self.next_session_id = self.next_session_id.saturating_add(1);
				let session_id = id.to_string();
				self.trace = SttTrace::start(&session_id);
				self.session_id = Some(session_id);
				self.reset_transcription_state_with_activity();

				self.set_state(SessionState::Showing, Some("Session started.".to_string()));
				self.set_state(SessionState::Listening, None);

				if let Some(session_id) = self.session_id.clone() {
					self.start_dictation(&session_id).await;
				}
			},
			SessionAction::Enter => match self.state {
				SessionState::Listening => {
					self.set_state(SessionState::Finalizing, None);

					if let Some(session_id) = self.session_id.clone() {
						let handle = self.cancel_stt(false);
						let trace = self.trace.clone();
						self.spawn_finalize_task(&session_id, handle, trace);
					}
				},
				SessionState::RewriteReady => {
					self.set_state(SessionState::Injecting, None);

					if let Some(session_id) = self.session_id.clone() {
						let text =
							self.rewrite_text.clone().unwrap_or_else(|| self.raw_text.clone());
						self.spawn_inject_task(&session_id, text);
					}
				},
				_ => {},
			},
			SessionAction::Escape => {
				let trace = self.trace.clone();
				let handle = if trace.is_some() {
					self.cancel_stt(false)
				} else {
					let _ = self.cancel_stt(true);
					None
				};
				if let Some(trace) = trace
					&& let Some(handle) = handle
				{
					self.spawn_trace_audio_task(handle, trace);
				}

				self.set_state(SessionState::Hidden, Some("Session cancelled.".to_string()));
				self.hide_window();
				self.trace = None;
				self.session_id = None;
				self.reset_transcription_state();
			},
			SessionAction::Rewrite => {
				if self.state == SessionState::RewriteReady
					&& let Some(session_id) = self.session_id.clone()
				{
					let input = self.raw_text.clone();
					self.start_rewrite(&session_id, input).await;
				}
			},
		}

		Ok(self.snapshot())
	}

	fn snapshot(&self) -> SessionSnapshot {
		SessionSnapshot {
			session_id: self.session_id.clone(),
			state: self.state,
			raw_text: self.raw_text.clone(),
			rewrite_text: self.rewrite_text.clone(),
		}
	}

	fn reset_transcription_state(&mut self) {
		self.raw_text.clear();
		self.stt_segments.clear();
		self.stt_live_text.clear();
		self.stt_live_has_window = false;
		self.rewrite_text = None;
		self.stt_revision = 0;
		self.committed_end_16k_samples = 0;
		self.window_generation = 0;
		self.window_job_id_last_scheduled = 0;
		self.window_job_id_last_applied = 0;
		self.merge = MergeState::default();
		self.stt_settings = None;
	}

	fn reset_transcription_state_with_activity(&mut self) {
		self.reset_transcription_state();
		self.last_activity_at = Instant::now();
	}

	fn set_state(&mut self, state: SessionState, reason: Option<String>) {
		self.state = state;
		let _ = self.state_tx.send(state);
		self.emit_state(reason);
	}

	fn emit_state(&self, reason: Option<String>) {
		let Some(session_id) = self.session_id.clone() else {
			return;
		};

		let Some(context) = self.context.as_ref() else {
			return;
		};

		context.emit_session_state(SessionStateEvent { session_id, state: self.state, reason });
	}

	fn hide_window(&self) {
		if let Some(context) = self.context.as_ref() {
			context.hide_overlay_window();
		}
	}

	fn show_window(&self) {
		if let Some(context) = self.context.as_ref() {
			context.show_overlay_window();
		}
	}

	fn cancel_stt(&mut self, abort: bool) -> Option<SttHandle> {
		let stt = self.stt_runtime.take()?;
		stt.cancel.cancel();

		if abort {
			stt.handle.abort();
			stt.updates_handle.abort();
			return None;
		}

		Some(stt.handle)
	}

	async fn start_dictation(&mut self, session_id: &str) {
		let Some(context) = self.context.clone() else {
			return;
		};

		let stt_settings = match context.stt_settings().await {
			Ok(settings) => settings,
			Err(err) => {
				self.set_error(session_id, err.clone(), false).await;
				return;
			},
		};

		if let Err(err) = stt_settings.validate() {
			self.set_error(session_id, err.clone(), false).await;
			return;
		}

		let recognizer = match context.get_sherpa(&stt_settings).await {
			Ok(recognizer) => recognizer,
			Err(err) => {
				context.set_engine_error(err.clone()).await;
				self.set_error(session_id, err.clone(), false).await;
				return;
			},
		};

		let stream = match recognizer.create_stream() {
			Ok(stream) => stream,
			Err(err) => {
				let err = AppError::new(
					"stt_stream_init_failed",
					format!("Failed to create the STT stream: {err}."),
				);
				self.set_error(session_id, err.clone(), false).await;
				return;
			},
		};

		let whisper_config = match stt::resolve_whisper_config(&stt_settings.whisper) {
			Ok(config) => config,
			Err(err) => {
				self.set_error(session_id, err.clone(), false).await;
				return;
			},
		};

		let whisper_ctx = match context.get_whisper(&stt_settings).await {
			Ok(ctx) => ctx,
			Err(err) => {
				context.set_engine_error(err.clone()).await;
				self.set_error(session_id, err.clone(), false).await;
				return;
			},
		};

		let window_profile =
			stt::WhisperDecodeProfile { best_of: stt_settings.profiles.window_best_of };
		let second_pass_profile =
			stt::WhisperDecodeProfile { best_of: stt_settings.profiles.second_pass_best_of };

		let (mic, recording) = match crate::audio::MicStream::open_default().await {
			Ok(mic) => mic,
			Err(err) => {
				self.set_error(session_id, err.clone(), true).await;
				return;
			},
		};

		let sample_rate = mic.sample_rate();
		let engine_generation = context.engine_generation();

		self.stt_settings = Some(stt_settings.clone());

		let init = DictationInit {
			stt_settings,
			recognizer,
			stream,
			whisper_config,
			whisper_ctx,
			window_profile,
			second_pass_profile,
			mic,
			recording,
			sample_rate,
			engine_generation,
		};

		let pipeline = DictationPipeline::start(init);
		let (cancel, mut update_rx, handle) = pipeline.split();
		let command_tx = self.command_tx.clone();
		let updates_handle = tauri::async_runtime::spawn(async move {
			while let Some(update) = update_rx.recv().await {
				if command_tx.send(SessionCommand::PipelineUpdate(update)).await.is_err() {
					return;
				}
			}
		});

		self.stt_runtime = Some(SttRuntime { cancel, handle, updates_handle });
	}

	fn spawn_finalize_task(
		&self,
		session_id: &str,
		handle: Option<SttHandle>,
		trace: Option<SttTrace>,
	) {
		let command_tx = self.command_tx.clone();
		let session_id = session_id.to_string();
		tauri::async_runtime::spawn(async move {
			if trace.is_some() {
				eprintln!("STT finalize started for session {session_id}.");
			}

			let result = finalize_stt_handle(&session_id, handle, trace).await;
			let _ = command_tx.send(SessionCommand::FinalizeReady { session_id, result }).await;
		});
	}

	fn spawn_trace_audio_task(&self, handle: SttHandle, trace: SttTrace) {
		tauri::async_runtime::spawn(async move {
			let recorded_audio = match handle.await {
				Ok(Ok(audio)) => audio,
				Ok(Err(err)) => {
					eprintln!("Failed to capture trace audio: {}.", err.message);
					return;
				},
				Err(err) => {
					eprintln!("Failed to capture trace audio: {err}.");
					return;
				},
			};

			let wav_path = trace.audio_path();
			let wav_path_display = wav_path.display().to_string();
			eprintln!("Writing STT trace audio for cancelled session: {}.", wav_path_display);
			let wav_path_for_worker = wav_path.clone();
			let _ = tauri::async_runtime::spawn_blocking(move || {
				if let Err(err) = write_recorded_audio_wav(&wav_path_for_worker, &recorded_audio) {
					eprintln!("Failed to write STT trace audio: {}.", err.message);
				}
			})
			.await;
			eprintln!(
				"Finished writing STT trace audio for cancelled session: {}.",
				wav_path_display
			);
		});
	}

	fn spawn_inject_task(&self, session_id: &str, text: String) {
		let command_tx = self.command_tx.clone();
		let session_id = session_id.to_string();
		let context = self.context.clone();
		tauri::async_runtime::spawn(async move {
			let result = match context {
				Some(context) => run_inject(context, text).await,
				None => Err(AppError::new(
					"session_context_missing",
					"Session context is not available.",
				)),
			};
			let _ = command_tx.send(SessionCommand::InjectDone { session_id, result }).await;
		});
	}

	async fn start_rewrite(&mut self, session_id: &str, input: String) {
		if input.trim().is_empty() {
			self.rewrite_text = Some(String::new());
			self.set_state(SessionState::RewriteReady, None);
			return;
		}

		self.set_state(SessionState::Rewriting, None);

		let command_tx = self.command_tx.clone();
		let session_id = session_id.to_string();
		let context = self.context.clone();
		tauri::async_runtime::spawn(async move {
			let result = match context {
				Some(context) => run_rewrite(context, &input).await,
				None => Err(AppError::new(
					"session_context_missing",
					"Session context is not available.",
				)),
			};
			let _ = command_tx
				.send(SessionCommand::RewriteDone { session_id, result, fallback_text: input })
				.await;
		});
	}

	async fn handle_pipeline_update(&mut self, update: AsrUpdate) {
		let Some(session_id) = self.session_id.clone() else {
			return;
		};

		let Some(stt_settings) = self.stt_settings.clone() else {
			return;
		};

		match update {
			AsrUpdate::SherpaPartial(text) => {
				self.set_stt_live_text(&session_id, &text, SttStrategy::VadChunk);
			},
			AsrUpdate::WindowScheduled(snapshot) => {
				self.note_window_scheduled(&session_id, &snapshot);
			},
			AsrUpdate::WindowResult { snapshot, result } => {
				self.apply_window_result(&session_id, snapshot, result, &stt_settings);
			},
			AsrUpdate::SegmentEnd {
				segment_id,
				sherpa_text,
				committed_end_16k_samples,
				window_generation_after,
			} => {
				self.commit_stt_segment_at(
					&session_id,
					segment_id,
					&sherpa_text,
					committed_end_16k_samples,
					window_generation_after,
					SttStrategy::VadChunk,
					&stt_settings,
				);
			},
			AsrUpdate::EndpointReset { window_generation_after } => {
				self.reset_live_tail_on_endpoint(
					&session_id,
					window_generation_after,
					SttStrategy::VadChunk,
				);
			},
			AsrUpdate::SecondPass { segment_id, text } => {
				self.replace_stt_segment_at(
					&session_id,
					segment_id,
					&text,
					SttStrategy::VadChunk,
					&stt_settings,
				);
			},
		}
	}

	async fn handle_finalize_ready(&mut self, session_id: &str, result: Result<(), AppError>) {
		if self.session_id.as_deref() != Some(session_id) {
			return;
		}

		if let Err(err) = result {
			self.set_error(session_id, err, true).await;
			return;
		}

		self.commit_stt_segment(session_id, SttStrategy::VadChunk);

		let final_text = self.raw_text.clone();
		self.set_final_text(session_id, final_text.clone());
		self.start_rewrite(session_id, final_text).await;
	}

	async fn handle_rewrite_done(
		&mut self,
		session_id: &str,
		result: Result<llm::RewriteResult, AppError>,
		fallback_text: String,
	) {
		if self.session_id.as_deref() != Some(session_id) {
			return;
		}

		match result {
			Ok(rewrite) => {
				self.rewrite_text = Some(rewrite.text.clone());

				if let Some(context) = self.context.as_ref() {
					context.emit_llm_rewrite(LlmRewriteEvent {
						session_id: session_id.to_string(),
						text: rewrite.text,
						model: rewrite.model,
					});
				}

				self.set_state(SessionState::RewriteReady, None);
			},
			Err(err) => {
				self.finish_rewrite_with_error(session_id, fallback_text, err).await;
			},
		}
	}

	async fn handle_inject_done(&mut self, session_id: &str, result: Result<(), AppError>) {
		if self.session_id.as_deref() != Some(session_id) {
			return;
		}

		match result {
			Ok(()) => {
				self.set_state(SessionState::Hidden, None);
				self.session_id = None;
				self.reset_transcription_state();
			},
			Err(err) => {
				self.set_state(SessionState::RewriteReady, Some(err.message.clone()));
				if let Some(context) = self.context.as_ref() {
					context.emit_error(ErrorEvent {
						session_id: Some(session_id.to_string()),
						code: err.code,
						message: err.message,
						recoverable: true,
					});
				}
				self.show_window();
			},
		}
	}

	async fn finish_rewrite_with_error(
		&mut self,
		session_id: &str,
		fallback_text: String,
		err: AppError,
	) {
		self.rewrite_text = Some(fallback_text);
		self.set_state(SessionState::RewriteReady, Some(err.message.clone()));
		if let Some(context) = self.context.as_ref() {
			context.emit_error(ErrorEvent {
				session_id: Some(session_id.to_string()),
				code: err.code,
				message: err.message,
				recoverable: true,
			});
		}
	}

	async fn set_error(&mut self, session_id: &str, err: AppError, recoverable: bool) {
		if self.session_id.as_deref() != Some(session_id) {
			return;
		}

		self.set_state(SessionState::Error, Some(err.message.clone()));
		if let Some(context) = self.context.as_ref() {
			context.emit_error(ErrorEvent {
				session_id: Some(session_id.to_string()),
				code: err.code,
				message: err.message,
				recoverable,
			});
		}
	}

	fn set_stt_live_text(&mut self, session_id: &str, text: &str, strategy: SttStrategy) {
		let incoming = text.trim();
		if incoming.is_empty() {
			return;
		}

		if self.session_id.as_deref() != Some(session_id) {
			return;
		}

		if self.stt_live_has_window && matches!(strategy, SttStrategy::VadChunk) {
			if let Some(trace) = self.trace.as_ref() {
				trace.emit(
					"stt_partial_ignored",
					None,
					Some(strategy),
					Some(incoming.to_string()),
					Self::build_trace_details(self, None),
				);
			}
			self.last_activity_at = Instant::now();
			return;
		}

		let next_live = incoming.to_string();
		if self.stt_live_text == next_live {
			return;
		}

		self.stt_live_text = next_live;
		self.raw_text = Self::build_stt_text(&self.stt_segments, &self.stt_live_text);
		self.bump_revision_and_emit_partial(session_id, strategy, None);
	}

	fn commit_stt_segment(&mut self, session_id: &str, strategy: SttStrategy) {
		if self.session_id.as_deref() != Some(session_id) {
			return;
		}

		let mut live = self.stt_live_text.trim().to_string();
		if live.is_empty() {
			return;
		}

		if let Some(settings) = self.stt_settings.as_ref() {
			let committed_text = Self::build_stt_text(&self.stt_segments, "");
			let language = &settings.whisper.language;
			let mode = token_mode_for_language(language, &live);

			let deduped = dedup_tail(
				&committed_text,
				&live,
				mode,
				settings.merge.overlap_k_words as usize,
				settings.merge.overlap_k_chars as usize,
			);

			let deduped = deduped.trim().to_string();
			if deduped.is_empty() {
				self.stt_live_text.clear();
				self.stt_live_has_window = false;
				self.merge.reset();
				self.raw_text = Self::build_stt_text(&self.stt_segments, "");
				self.bump_revision_and_emit_partial(session_id, strategy, None);

				return;
			}

			live = deduped;
		}

		self.stt_segments.push(live);

		self.stt_live_text.clear();
		self.stt_live_has_window = false;
		self.merge.reset();

		self.raw_text = Self::build_stt_text(&self.stt_segments, "");
		self.bump_revision_and_emit_partial(session_id, strategy, None);
	}

	#[allow(clippy::too_many_arguments)]
	fn commit_stt_segment_at(
		&mut self,
		session_id: &str,
		segment_id: u64,
		text: &str,
		committed_end_16k_samples: u64,
		window_generation_after: u64,
		strategy: SttStrategy,
		settings: &SttSettings,
	) {
		if segment_id == 0 {
			return;
		}

		if self.session_id.as_deref() != Some(session_id) {
			return;
		}

		let idx = segment_id.saturating_sub(1) as usize;
		if self.stt_segments.len() <= idx {
			self.stt_segments.resize(idx + 1, String::new());
		}

		let sherpa_text = text.trim();
		let live_text = self.stt_live_text.trim();
		let mut provisional =
			choose_segment_provisional_text(sherpa_text, live_text, self.stt_live_has_window);
		if idx > 0 {
			let committed_text = Self::build_stt_text(&self.stt_segments[..idx], "");
			let language = &settings.whisper.language;
			let mode = token_mode_for_language(language, &provisional);

			let deduped = dedup_tail(
				&committed_text,
				&provisional,
				mode,
				settings.merge.overlap_k_words as usize,
				settings.merge.overlap_k_chars as usize,
			);

			if !deduped.trim().is_empty() {
				provisional = deduped;
			}
		}
		self.stt_segments[idx] = provisional.clone();

		if let Some(trace) = self.trace.as_ref() {
			trace.emit(
				"segment_commit",
				None,
				Some(strategy),
				Some(provisional.clone()),
				Self::build_trace_details(self, Some(segment_id)),
			);
		}
		self.stt_live_text.clear();
		self.stt_live_has_window = false;
		self.merge.reset();
		self.committed_end_16k_samples = committed_end_16k_samples;
		self.window_generation = window_generation_after;
		self.window_job_id_last_scheduled = 0;
		self.window_job_id_last_applied = 0;
		self.raw_text = Self::build_stt_text(&self.stt_segments, "");
		self.bump_revision_and_emit_partial(session_id, strategy, Some(segment_id));
	}

	fn reset_live_tail_on_endpoint(
		&mut self,
		session_id: &str,
		window_generation_after: u64,
		strategy: SttStrategy,
	) {
		if self.session_id.as_deref() != Some(session_id) {
			return;
		}

		if self.stt_live_text.trim().is_empty() {
			self.window_generation = window_generation_after;
			self.window_job_id_last_scheduled = 0;
			self.window_job_id_last_applied = 0;
			self.merge.reset();
			self.stt_live_has_window = false;

			if let Some(trace) = self.trace.as_ref() {
				trace.emit(
					"endpoint_reset",
					None,
					Some(strategy),
					None,
					Self::build_trace_details(self, None),
				);
			}
			return;
		}

		self.stt_live_text.clear();
		self.stt_live_has_window = false;
		self.window_generation = window_generation_after;
		self.window_job_id_last_scheduled = 0;
		self.window_job_id_last_applied = 0;
		self.merge.reset();

		self.raw_text = Self::build_stt_text(&self.stt_segments, "");
		self.bump_revision_and_emit_partial(session_id, strategy, None);
	}

	fn note_window_scheduled(&mut self, session_id: &str, snapshot: &stt::WindowJobSnapshot) {
		if self.session_id.as_deref() != Some(session_id) {
			return;
		}

		if snapshot.window_generation != self.window_generation {
			return;
		}

		if snapshot.job_id > self.window_job_id_last_scheduled {
			self.window_job_id_last_scheduled = snapshot.job_id;
		}

		if let Some(trace) = self.trace.as_ref() {
			trace.emit("window_scheduled", None, None, None, Self::build_trace_details(self, None));
		}
	}

	fn apply_window_result(
		&mut self,
		session_id: &str,
		snapshot: stt::WindowJobSnapshot,
		result: stt::WhisperDecodeResult,
		settings: &SttSettings,
	) {
		if !settings.window.enabled {
			return;
		}

		let Some(context) = self.context.as_ref() else {
			return;
		};

		if snapshot.engine_generation != context.engine_generation() {
			return;
		}

		if self.session_id.as_deref() != Some(session_id) {
			return;
		}

		if snapshot.window_generation != self.window_generation {
			return;
		}

		if snapshot.job_id <= self.window_job_id_last_applied {
			return;
		}

		let latest = self.window_job_id_last_scheduled;
		if latest > 0 && snapshot.job_id.saturating_add(1) < latest {
			return;
		}

		let committed_text = Self::build_stt_text(&self.stt_segments, "");
		let language = &settings.whisper.language;
		let mode = token_mode_for_language(language, &result.text);

		let window_tail = if result.has_timestamps {
			extract_window_tail_text(
				&snapshot,
				self.committed_end_16k_samples,
				language,
				&result.segments,
			)
		} else {
			result.text.clone()
		};

		let deduped_tail = dedup_tail(
			&committed_text,
			&window_tail,
			mode,
			settings.merge.overlap_k_words as usize,
			settings.merge.overlap_k_chars as usize,
		);

		let stable_tail = self.merge.apply_candidate(
			&deduped_tail,
			mode,
			settings.merge.stable_ticks as usize,
			settings.merge.rollback_threshold_tokens as usize,
		);

		let next_live = stable_tail.trim().to_string();
		if next_live.is_empty() {
			return;
		}

		if self.stt_live_text == next_live {
			self.window_job_id_last_applied = snapshot.job_id;
			self.stt_live_has_window = true;

			if let Some(trace) = self.trace.as_ref() {
				trace.emit(
					"window_result_no_change",
					None,
					Some(SttStrategy::SlidingWindow),
					None,
					Self::build_trace_details(self, None),
				);
			}
			return;
		}

		self.window_job_id_last_applied = snapshot.job_id;
		self.stt_live_has_window = true;
		self.stt_live_text = next_live;
		self.raw_text = Self::build_stt_text(&self.stt_segments, &self.stt_live_text);
		self.bump_revision_and_emit_partial(session_id, SttStrategy::SlidingWindow, None);
	}

	fn replace_stt_segment_at(
		&mut self,
		session_id: &str,
		segment_id: u64,
		text: &str,
		strategy: SttStrategy,
		settings: &SttSettings,
	) {
		let incoming = text.trim();
		if incoming.is_empty() || segment_id == 0 {
			return;
		}

		if self.session_id.as_deref() != Some(session_id) {
			return;
		}

		let idx = segment_id.saturating_sub(1) as usize;
		if self.stt_segments.len() <= idx {
			self.stt_segments.resize(idx + 1, String::new());
		}

		let current = self.stt_segments[idx].trim();
		let live = self.stt_live_text.trim();

		let mut incoming = collapse_leading_duplicate_word(incoming);
		if incoming.is_empty() {
			return;
		}

		if idx > 0 {
			let committed_text = Self::build_stt_text(&self.stt_segments[..idx], "");
			let language = &settings.whisper.language;
			let mode = token_mode_for_language(language, &incoming);

			let deduped = dedup_tail(
				&committed_text,
				&incoming,
				mode,
				settings.merge.overlap_k_words as usize,
				settings.merge.overlap_k_chars as usize,
			);

			if !deduped.trim().is_empty() {
				incoming = deduped;
			}
		}

		{
			let language = &settings.whisper.language;
			let mode = token_mode_for_language(language, &incoming);

			if !should_accept_second_pass_replacement(current, &incoming, mode) {
				if let Some(trace) = self.trace.as_ref() {
					trace.emit(
						"second_pass_rejected",
						None,
						Some(strategy),
						Some(incoming.clone()),
						Self::build_trace_details(self, Some(segment_id)),
					);
				}
				return;
			}
		}

		if current.is_empty()
			&& !live.is_empty()
			&& let (Some(incoming_first), Some(live_first)) =
				(incoming.split_whitespace().next(), live.split_whitespace().next())
			&& !leading_words_compatible(incoming_first, live_first)
		{
			return;
		}

		if self.stt_segments[idx] == incoming {
			return;
		}

		self.stt_segments[idx] = incoming;
		self.raw_text = Self::build_stt_text(&self.stt_segments, &self.stt_live_text);
		self.bump_revision_and_emit_partial(session_id, strategy, Some(segment_id));
	}

	fn set_final_text(&mut self, session_id: &str, text: String) {
		if self.session_id.as_deref() != Some(session_id) {
			return;
		}

		self.raw_text = text.clone();
		if let Some(context) = self.context.as_ref() {
			context.emit_stt_final(SttFinalEvent { session_id: session_id.to_string(), text });
		}

		if let Some(trace) = self.trace.as_ref() {
			trace.emit(
				"stt_final",
				None,
				None,
				Some(self.raw_text.clone()),
				Self::build_trace_details(self, None),
			);
		}
	}

	fn bump_revision_and_emit_partial(
		&mut self,
		session_id: &str,
		strategy: SttStrategy,
		segment_id: Option<u64>,
	) {
		self.last_activity_at = Instant::now();
		self.stt_revision = self.stt_revision.saturating_add(1);

		if let Some(context) = self.context.as_ref() {
			context.emit_stt_partial(SttPartialEvent {
				session_id: session_id.to_string(),
				revision: self.stt_revision,
				text: self.raw_text.clone(),
				strategy,
			});
		}

		if let Some(trace) = self.trace.as_ref() {
			trace.emit(
				"stt_partial",
				Some(self.stt_revision),
				Some(strategy),
				Some(self.raw_text.clone()),
				Self::build_trace_details(self, segment_id),
			);
		}
	}

	fn build_stt_text(segments: &[String], live_text: &str) -> String {
		let mut out = String::new();

		for segment in segments {
			let trimmed = segment.trim();
			if trimmed.is_empty() {
				continue;
			}
			if !out.is_empty() {
				out.push(' ');
			}
			out.push_str(trimmed);
		}

		let live_trimmed = live_text.trim();
		if !live_trimmed.is_empty() {
			if !out.is_empty() {
				out.push(' ');
			}
			out.push_str(live_trimmed);
		}

		out
	}

	fn build_trace_details(inner: &Self, segment_id: Option<u64>) -> TraceDetails {
		TraceDetails {
			segment_id,
			live_has_window: Some(inner.stt_live_has_window),
			window_generation: Some(inner.window_generation),
			window_job_id_last_scheduled: Some(inner.window_job_id_last_scheduled),
			window_job_id_last_applied: Some(inner.window_job_id_last_applied),
		}
	}
}

struct SttRuntime {
	cancel: CancellationToken,
	handle: SttHandle,
	updates_handle: tauri::async_runtime::JoinHandle<()>,
}

async fn finalize_stt_handle(
	session_id: &str,
	handle: Option<SttHandle>,
	trace: Option<SttTrace>,
) -> Result<(), AppError> {
	let mut recorded_audio: Option<crate::audio::RecordedAudio> = None;
	if let Some(handle) = handle {
		match handle.await {
			Ok(Ok(audio)) => {
				recorded_audio = Some(audio);
			},
			Ok(Err(err)) => {
				return Err(err);
			},
			Err(err) => {
				return Err(AppError::new(
					"stt_task_failed",
					format!("The STT task failed: {err}."),
				));
			},
		}
	}

	if let (Some(audio), Some(trace)) = (recorded_audio, trace) {
		let wav_path = trace.audio_path();
		let wav_path_display = wav_path.display().to_string();
		eprintln!("Writing STT trace audio for session {session_id}: {}.", wav_path_display);
		let wav_path_for_worker = wav_path.clone();
		let handle = tauri::async_runtime::spawn_blocking(move || {
			if let Err(err) = write_recorded_audio_wav(&wav_path_for_worker, &audio) {
				eprintln!("Failed to write STT trace audio: {}.", err.message);
			}
		});
		let _ = handle.await;
		eprintln!(
			"Finished writing STT trace audio for session {session_id}: {}.",
			wav_path_display
		);
	}

	Ok(())
}

async fn run_rewrite(
	context: Arc<SessionContext>,
	input: &str,
) -> Result<llm::RewriteResult, AppError> {
	let llm_settings = context.llm_settings().await?;

	let Some(llm_settings) = llm_settings else {
		return Err(AppError::new(
			"llm_not_configured",
			"LLM rewrite is not configured. Open Settings and set base_url, model, and API key.",
		));
	};

	llm::rewrite(&llm_settings, input).await
}

async fn run_inject(context: Arc<SessionContext>, text: String) -> Result<(), AppError> {
	context.hide_overlay_window();
	tokio::time::sleep(Duration::from_millis(80)).await;

	let platform = context.platform();
	let text_for_inject = text.clone();
	let inject_result = tauri::async_runtime::spawn_blocking(move || {
		platform.inject_text_via_paste(&text_for_inject)
	})
	.await;

	match inject_result {
		Ok(result) => result,
		Err(err) => Err(AppError::new(
			"text_injection_failed",
			format!("Text injection task failed: {err}."),
		)),
	}
}
