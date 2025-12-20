use std::sync::{
	Arc,
	atomic::{AtomicU64, Ordering},
};

use serde::Serialize;
use tokio::sync::RwLock;

use crate::{
	error::AppError,
	events::{EngineState, EngineStateEvent, emit_engine_state},
	settings::SttSettings,
	stt,
};

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApplyLevel {
	SoftApplied,
	Reloaded,
	#[allow(dead_code)]
	RestartRequired,
}

pub struct EngineManager {
	engine_generation: AtomicU64,
	state: RwLock<EngineState>,
	sherpa: RwLock<Option<sherpa_onnx::OnlineRecognizer>>,
	whisper: RwLock<Option<Arc<whisper_rs::WhisperContext>>>,
	loaded_settings: RwLock<Option<SttSettings>>,
}

impl EngineManager {
	pub fn new() -> Self {
		Self {
			engine_generation: AtomicU64::new(1),
			state: RwLock::new(EngineState::Ready),
			sherpa: RwLock::new(None),
			whisper: RwLock::new(None),
			loaded_settings: RwLock::new(None),
		}
	}

	pub fn current_engine_generation(&self) -> u64 {
		self.engine_generation.load(Ordering::SeqCst)
	}

	#[allow(dead_code)]
	pub async fn state(&self) -> EngineState {
		*self.state.read().await
	}

	pub async fn classify_apply_level(&self, old: &SttSettings, new: &SttSettings) -> ApplyLevel {
		if old.whisper.model_path != new.whisper.model_path
			|| old.whisper.force_gpu != new.whisper.force_gpu
			|| old.sherpa.model_dir != new.sherpa.model_dir
			|| old.sherpa.provider != new.sherpa.provider
			|| old.sherpa.num_threads != new.sherpa.num_threads
			|| old.sherpa.decoding_method != new.sherpa.decoding_method
			|| old.sherpa.max_active_paths != new.sherpa.max_active_paths
			|| old.sherpa.rule1_min_trailing_silence != new.sherpa.rule1_min_trailing_silence
			|| old.sherpa.rule2_min_trailing_silence != new.sherpa.rule2_min_trailing_silence
			|| old.sherpa.rule3_min_utterance_length != new.sherpa.rule3_min_utterance_length
			|| old.sherpa.prefer_int8 != new.sherpa.prefer_int8
			|| old.sherpa.use_int8_decoder != new.sherpa.use_int8_decoder
		{
			return ApplyLevel::Reloaded;
		}

		ApplyLevel::SoftApplied
	}

	pub async fn apply_settings(
		&self,
		app: &tauri::AppHandle,
		old: &SttSettings,
		new: &SttSettings,
	) -> Result<ApplyLevel, AppError> {
		new.validate()?;

		let apply_level = self.classify_apply_level(old, new).await;
		match apply_level {
			ApplyLevel::SoftApplied => {
				*self.loaded_settings.write().await = Some(new.clone());
				Ok(ApplyLevel::SoftApplied)
			},
			ApplyLevel::Reloaded => {
				self.reload_engine_objects(app, new, "Settings changed.").await?;
				Ok(ApplyLevel::Reloaded)
			},
			ApplyLevel::RestartRequired => Ok(ApplyLevel::RestartRequired),
		}
	}

	pub async fn get_sherpa(
		&self,
		app: &tauri::AppHandle,
		settings: &SttSettings,
	) -> Result<sherpa_onnx::OnlineRecognizer, AppError> {
		self.ensure_loaded(app, settings).await?;

		let guard = self.sherpa.read().await;
		let Some(recognizer) = guard.clone() else {
			return Err(AppError::new("engine_not_ready", "The speech engine is not ready."));
		};

		Ok(recognizer)
	}

	pub async fn get_whisper(
		&self,
		app: &tauri::AppHandle,
		settings: &SttSettings,
	) -> Result<Arc<whisper_rs::WhisperContext>, AppError> {
		self.ensure_loaded(app, settings).await?;

		let guard = self.whisper.read().await;
		let Some(ctx) = guard.clone() else {
			return Err(AppError::new("engine_not_ready", "The speech engine is not ready."));
		};

		Ok(ctx)
	}

	async fn ensure_loaded(
		&self,
		app: &tauri::AppHandle,
		settings: &SttSettings,
	) -> Result<(), AppError> {
		{
			let loaded = self.loaded_settings.read().await;
			let sherpa_ready = self.sherpa.read().await.is_some();
			let whisper_ready = self.whisper.read().await.is_some();
			if sherpa_ready && whisper_ready && loaded.as_ref().is_some_and(|s| s == settings) {
				return Ok(());
			}
		}

		self.reload_engine_objects(app, settings, "Loading speech engine.").await
	}

	async fn reload_engine_objects(
		&self,
		app: &tauri::AppHandle,
		settings: &SttSettings,
		reason: &str,
	) -> Result<(), AppError> {
		{
			let mut st = self.state.write().await;
			*st = EngineState::Reloading;
			emit_engine_state(
				app,
				EngineStateEvent { state: *st, reason: Some(reason.to_string()) },
			);
		}

		self.engine_generation.fetch_add(1, Ordering::SeqCst);

		{
			let mut sherpa_guard = self.sherpa.write().await;
			*sherpa_guard = None;
		}
		{
			let mut whisper_guard = self.whisper.write().await;
			*whisper_guard = None;
		}

		let sherpa_settings = settings.sherpa.clone();
		let whisper_settings = settings.whisper.clone();

		let sherpa = tauri::async_runtime::spawn_blocking(move || {
			let config = stt::resolve_sherpa_config(&sherpa_settings)?;
			sherpa_onnx::OnlineRecognizer::new(config).map_err(|err| {
				AppError::new(
					"stt_init_failed",
					format!("Failed to initialize sherpa-onnx STT: {err}."),
				)
			})
		})
		.await
		.map_err(|err| {
			AppError::new("stt_init_failed", format!("STT initialization task failed: {err}."))
		})??;

		let whisper = tauri::async_runtime::spawn_blocking(move || {
			stt::load_whisper_context(&whisper_settings)
		})
		.await
		.map_err(|err| {
			AppError::new(
				"whisper_init_failed",
				format!("Whisper initialization task failed: {err}."),
			)
		})??;

		{
			let mut sherpa_guard = self.sherpa.write().await;
			*sherpa_guard = Some(sherpa);
		}
		{
			let mut whisper_guard = self.whisper.write().await;
			*whisper_guard = Some(Arc::new(whisper));
		}
		{
			let mut loaded_guard = self.loaded_settings.write().await;
			*loaded_guard = Some(settings.clone());
		}

		{
			let mut st = self.state.write().await;
			*st = EngineState::Ready;
			emit_engine_state(app, EngineStateEvent { state: *st, reason: None });
		}

		Ok(())
	}

	pub async fn set_error(&self, app: &tauri::AppHandle, err: AppError) {
		{
			let mut st = self.state.write().await;
			*st = EngineState::Error;
			emit_engine_state(
				app,
				EngineStateEvent { state: *st, reason: Some(err.message.clone()) },
			);
		}
	}
}
