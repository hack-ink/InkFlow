use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use tauri::Manager as _;
use tokio::sync::{OnceCell, RwLock};

use crate::error::AppError;

const SETTINGS_FILENAME: &str = "settings.json";

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
struct SettingsFile {
	llm: LlmSettingsFile,
	session: SessionSettingsFile,
	stt: SttSettings,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
struct SessionSettingsFile {
	silence_timeout_ms: u64,
}

impl Default for SessionSettingsFile {
	fn default() -> Self {
		Self { silence_timeout_ms: 2500 }
	}
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
struct LlmSettingsFile {
	base_url: String,
	api_key: String,
	model: String,
	temperature: f32,
	system_prompt: String,
}

impl Default for LlmSettingsFile {
	fn default() -> Self {
		Self {
			base_url: "https://api.openai.com/v1".into(),
			api_key: String::new(),
			model: "gpt-4o-mini".into(),
			temperature: 1.0,
			system_prompt: "Rewrite the user text to be clear, grammatical, and concise. Preserve the original meaning."
				.to_string(),
		}
	}
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SttSettings {
	pub sherpa: SherpaSettings,
	pub whisper: WhisperSettings,
	pub window: WhisperWindowSettings,
	pub merge: MergeSettings,
	pub profiles: WhisperProfiles,
}

impl SttSettings {
	pub fn validate(&self) -> Result<(), AppError> {
		self.sherpa.validate()?;
		self.whisper.validate()?;
		self.window.validate()?;
		self.merge.validate()?;
		self.profiles.validate()?;

		if self.window.enabled && self.window.window_ms < self.window.step_ms {
			return Err(AppError::new(
				"settings_invalid",
				"window_ms must be greater than or equal to step_ms.",
			));
		}

		Ok(())
	}
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SherpaSettings {
	pub model_dir: String,
	pub provider: String,
	pub num_threads: Option<i32>,
	pub decoding_method: String,
	pub max_active_paths: i32,
	pub rule1_min_trailing_silence: f32,
	pub rule2_min_trailing_silence: f32,
	pub rule3_min_utterance_length: f32,
	pub prefer_int8: bool,
	pub use_int8_decoder: bool,
	pub chunk_ms: u32,
}

impl Default for SherpaSettings {
	fn default() -> Self {
		Self {
			model_dir: String::new(),
			provider: "cpu".into(),
			num_threads: None,
			decoding_method: "greedy_search".into(),
			max_active_paths: 4,
			rule1_min_trailing_silence: 2.4,
			rule2_min_trailing_silence: 1.2,
			rule3_min_utterance_length: 300.0,
			prefer_int8: true,
			use_int8_decoder: false,
			chunk_ms: 170,
		}
	}
}

impl SherpaSettings {
	fn validate(&self) -> Result<(), AppError> {
		if self.provider.trim().is_empty() {
			return Err(AppError::new("settings_invalid", "Sherpa provider must not be empty."));
		}

		match self.decoding_method.as_str() {
			"greedy_search" | "modified_beam_search" => {},
			other => {
				return Err(AppError::new(
					"settings_invalid",
					format!(
						"Invalid sherpa decoding method: {other:?}. Expected \"greedy_search\" or \"modified_beam_search\"."
					),
				));
			},
		}

		if let Some(threads) = self.num_threads
			&& threads <= 0
		{
			return Err(AppError::new(
				"settings_invalid",
				"Sherpa num_threads must be a positive integer when set.",
			));
		}

		if self.max_active_paths <= 0 {
			return Err(AppError::new(
				"settings_invalid",
				"Sherpa max_active_paths must be a positive integer.",
			));
		}

		if !self.rule1_min_trailing_silence.is_finite() || self.rule1_min_trailing_silence <= 0.0 {
			return Err(AppError::new(
				"settings_invalid",
				"Sherpa rule1_min_trailing_silence must be a positive, finite number.",
			));
		}

		if !self.rule2_min_trailing_silence.is_finite() || self.rule2_min_trailing_silence <= 0.0 {
			return Err(AppError::new(
				"settings_invalid",
				"Sherpa rule2_min_trailing_silence must be a positive, finite number.",
			));
		}

		if !self.rule3_min_utterance_length.is_finite() || self.rule3_min_utterance_length <= 0.0 {
			return Err(AppError::new(
				"settings_invalid",
				"Sherpa rule3_min_utterance_length must be a positive, finite number.",
			));
		}

		if self.chunk_ms < 40 || self.chunk_ms > 400 {
			return Err(AppError::new(
				"settings_invalid",
				"Sherpa chunk_ms must be between 40 and 400.",
			));
		}

		Ok(())
	}
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WhisperSettings {
	pub model_path: String,
	pub language: String,
	pub num_threads: Option<i32>,
	pub force_gpu: Option<bool>,
}

impl Default for WhisperSettings {
	fn default() -> Self {
		Self {
			model_path: String::new(),
			language: "en".into(),
			num_threads: None,
			force_gpu: None,
		}
	}
}

impl WhisperSettings {
	fn validate(&self) -> Result<(), AppError> {
		if self.language.contains('\0') {
			return Err(AppError::new(
				"settings_invalid",
				"Whisper language must not contain NUL bytes.",
			));
		}

		if let Some(threads) = self.num_threads
			&& threads <= 0
		{
			return Err(AppError::new(
				"settings_invalid",
				"Whisper num_threads must be a positive integer when set.",
			));
		}

		Ok(())
	}
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WhisperWindowSettings {
	pub enabled: bool,
	pub window_ms: u64,
	pub step_ms: u64,
	pub context_ms: u64,
	pub min_mean_abs: f32,
	pub emit_every: u32,
}

impl Default for WhisperWindowSettings {
	fn default() -> Self {
		Self {
			enabled: true,
			window_ms: 4000,
			step_ms: 400,
			context_ms: 800,
			min_mean_abs: 0.001,
			emit_every: 1,
		}
	}
}

impl WhisperWindowSettings {
	fn validate(&self) -> Result<(), AppError> {
		if self.step_ms == 0 {
			return Err(AppError::new("settings_invalid", "step_ms must be greater than zero."));
		}

		if self.window_ms < 100 {
			return Err(AppError::new("settings_invalid", "window_ms must be at least 100."));
		}

		if !self.min_mean_abs.is_finite() || self.min_mean_abs < 0.0 {
			return Err(AppError::new(
				"settings_invalid",
				"min_mean_abs must be a finite number greater than or equal to zero.",
			));
		}

		if self.emit_every == 0 {
			return Err(AppError::new("settings_invalid", "emit_every must be greater than zero."));
		}

		Ok(())
	}
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MergeSettings {
	pub stable_ticks: u32,
	pub rollback_threshold_tokens: u32,
	pub overlap_k_words: u32,
	pub overlap_k_chars: u32,
}

impl Default for MergeSettings {
	fn default() -> Self {
		Self {
			stable_ticks: 3,
			rollback_threshold_tokens: 8,
			overlap_k_words: 30,
			overlap_k_chars: 100,
		}
	}
}

impl MergeSettings {
	fn validate(&self) -> Result<(), AppError> {
		if self.stable_ticks == 0 {
			return Err(AppError::new(
				"settings_invalid",
				"stable_ticks must be greater than zero.",
			));
		}

		if self.overlap_k_words == 0 || self.overlap_k_chars == 0 {
			return Err(AppError::new(
				"settings_invalid",
				"Overlap limits must be greater than zero.",
			));
		}

		Ok(())
	}
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WhisperProfiles {
	pub window_best_of: u8,
	pub second_pass_best_of: u8,
}

impl Default for WhisperProfiles {
	fn default() -> Self {
		Self { window_best_of: 1, second_pass_best_of: 5 }
	}
}

impl WhisperProfiles {
	fn validate(&self) -> Result<(), AppError> {
		for (name, value) in [
			("window_best_of", self.window_best_of),
			("second_pass_best_of", self.second_pass_best_of),
		] {
			if value == 0 || value > 8 {
				return Err(AppError::new(
					"settings_invalid",
					format!("{name} must be within 1..=8."),
				));
			}
		}

		Ok(())
	}
}

#[derive(Clone, Debug, Serialize)]
pub struct SettingsPublic {
	pub llm: LlmSettingsPublic,
	pub session: SessionSettingsPublic,
	pub stt: SttSettings,
}

#[derive(Clone, Debug, Serialize)]
pub struct LlmSettingsPublic {
	pub base_url: String,
	pub model: String,
	pub temperature: f32,
	pub system_prompt: String,
	pub has_api_key: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionSettingsPublic {
	pub silence_timeout_ms: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SettingsPatch {
	pub llm: Option<LlmSettingsPatch>,
	pub session: Option<SessionSettingsPatch>,
	pub stt: Option<SttSettingsPatch>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LlmSettingsPatch {
	pub base_url: Option<String>,
	pub api_key: Option<String>,
	pub model: Option<String>,
	pub temperature: Option<f32>,
	pub system_prompt: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SessionSettingsPatch {
	pub silence_timeout_ms: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SttSettingsPatch {
	pub sherpa: Option<SherpaSettingsPatch>,
	pub whisper: Option<WhisperSettingsPatch>,
	pub window: Option<WhisperWindowSettingsPatch>,
	pub merge: Option<MergeSettingsPatch>,
	pub profiles: Option<WhisperProfilesPatch>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SherpaSettingsPatch {
	pub model_dir: Option<String>,
	pub provider: Option<String>,
	pub num_threads: Option<i32>,
	pub decoding_method: Option<String>,
	pub max_active_paths: Option<i32>,
	pub rule1_min_trailing_silence: Option<f32>,
	pub rule2_min_trailing_silence: Option<f32>,
	pub rule3_min_utterance_length: Option<f32>,
	pub prefer_int8: Option<bool>,
	pub use_int8_decoder: Option<bool>,
	pub chunk_ms: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct WhisperSettingsPatch {
	pub model_path: Option<String>,
	pub language: Option<String>,
	pub num_threads: Option<i32>,
	pub force_gpu: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct WhisperWindowSettingsPatch {
	pub enabled: Option<bool>,
	pub window_ms: Option<u64>,
	pub step_ms: Option<u64>,
	pub context_ms: Option<u64>,
	pub min_mean_abs: Option<f32>,
	pub emit_every: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MergeSettingsPatch {
	pub stable_ticks: Option<u32>,
	pub rollback_threshold_tokens: Option<u32>,
	pub overlap_k_words: Option<u32>,
	pub overlap_k_chars: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct WhisperProfilesPatch {
	pub window_best_of: Option<u8>,
	pub second_pass_best_of: Option<u8>,
}

#[derive(Clone, Debug)]
pub struct LlmSettingsResolved {
	pub base_url: String,
	pub api_key: String,
	pub model: String,
	pub temperature: f32,
	pub system_prompt: String,
}

pub struct SettingsStore {
	loaded: OnceCell<()>,
	inner: RwLock<SettingsFile>,
}

pub struct SettingsApplyResult {
	pub public: SettingsPublic,
	pub old_stt: SttSettings,
	pub new_stt: SttSettings,
}

impl SettingsStore {
	pub fn new() -> Self {
		Self { loaded: OnceCell::new(), inner: RwLock::new(SettingsFile::default()) }
	}

	pub async fn get_public(&self, app: &tauri::AppHandle) -> Result<SettingsPublic, AppError> {
		self.ensure_loaded(app).await?;
		let inner = self.inner.read().await;
		Ok(SettingsPublic {
			llm: LlmSettingsPublic {
				base_url: inner.llm.base_url.clone(),
				model: inner.llm.model.clone(),
				temperature: inner.llm.temperature,
				system_prompt: inner.llm.system_prompt.clone(),
				has_api_key: !inner.llm.api_key.is_empty(),
			},
			session: SessionSettingsPublic { silence_timeout_ms: inner.session.silence_timeout_ms },
			stt: inner.stt.clone(),
		})
	}

	pub async fn update(
		&self,
		app: &tauri::AppHandle,
		patch: SettingsPatch,
	) -> Result<SettingsPublic, AppError> {
		let res = self.apply_patch(app, patch).await?;
		Ok(res.public)
	}

	pub async fn apply_patch(
		&self,
		app: &tauri::AppHandle,
		patch: SettingsPatch,
	) -> Result<SettingsApplyResult, AppError> {
		self.ensure_loaded(app).await?;

		let (old_stt, public) = {
			let mut inner = self.inner.write().await;
			let old_stt = inner.stt.clone();
			let mut next = inner.clone();
			apply_patch_to_settings(&mut next, patch)?;

			*inner = next;
			let public = SettingsPublic {
				llm: LlmSettingsPublic {
					base_url: inner.llm.base_url.clone(),
					model: inner.llm.model.clone(),
					temperature: inner.llm.temperature,
					system_prompt: inner.llm.system_prompt.clone(),
					has_api_key: !inner.llm.api_key.is_empty(),
				},
				session: SessionSettingsPublic {
					silence_timeout_ms: inner.session.silence_timeout_ms,
				},
				stt: inner.stt.clone(),
			};
			(old_stt, public)
		};

		self.save_to_disk(app).await?;

		let new_stt = public.stt.clone();
		Ok(SettingsApplyResult { public, old_stt, new_stt })
	}

	pub async fn llm_resolved(
		&self,
		app: &tauri::AppHandle,
	) -> Result<Option<LlmSettingsResolved>, AppError> {
		self.ensure_loaded(app).await?;
		let inner = self.inner.read().await;
		let llm = &inner.llm;
		if llm.api_key.is_empty() || llm.base_url.trim().is_empty() || llm.model.trim().is_empty() {
			return Ok(None);
		}

		Ok(Some(LlmSettingsResolved {
			base_url: llm.base_url.clone(),
			api_key: llm.api_key.clone(),
			model: llm.model.clone(),
			temperature: llm.temperature,
			system_prompt: llm.system_prompt.clone(),
		}))
	}

	#[allow(dead_code)]
	pub async fn silence_timeout_ms(&self, app: &tauri::AppHandle) -> Result<u64, AppError> {
		self.ensure_loaded(app).await?;
		let inner = self.inner.read().await;
		Ok(inner.session.silence_timeout_ms)
	}

	pub async fn stt(&self, app: &tauri::AppHandle) -> Result<SttSettings, AppError> {
		self.ensure_loaded(app).await?;
		let inner = self.inner.read().await;
		Ok(inner.stt.clone())
	}

	async fn ensure_loaded(&self, app: &tauri::AppHandle) -> Result<(), AppError> {
		self.loaded
			.get_or_try_init(|| async {
				let settings = Self::load_from_disk(app)?;
				*self.inner.write().await = settings;
				Ok(())
			})
			.await?;
		Ok(())
	}

	fn load_from_disk(app: &tauri::AppHandle) -> Result<SettingsFile, AppError> {
		let path = Self::settings_path(app)?;
		if !path.exists() {
			return Ok(SettingsFile::default());
		}

		let raw = fs::read_to_string(&path).map_err(|err| {
			AppError::new("settings_read_failed", format!("Failed to read settings: {err}."))
		})?;

		match serde_json::from_str::<SettingsFile>(&raw) {
			Ok(settings) => Ok(settings),
			Err(err) => {
				let corrupt_path = path.with_extension("json.corrupt");
				let _ = fs::rename(&path, &corrupt_path);
				eprintln!("Failed to parse settings file. Using defaults instead. Error: {err}.",);
				Ok(SettingsFile::default())
			},
		}
	}

	async fn save_to_disk(&self, app: &tauri::AppHandle) -> Result<(), AppError> {
		let path = Self::settings_path(app)?;
		if let Some(parent) = path.parent() {
			fs::create_dir_all(parent).map_err(|err| {
				AppError::new(
					"settings_dir_create_failed",
					format!("Failed to create the settings directory: {err}."),
				)
			})?;
		}

		let inner = self.inner.read().await;
		let serialized = serde_json::to_string_pretty(&*inner).map_err(|err| {
			AppError::new(
				"settings_serialize_failed",
				format!("Failed to serialize settings: {err}."),
			)
		})?;

		let tmp_path = path.with_extension("json.tmp");
		fs::write(&tmp_path, serialized).map_err(|err| {
			AppError::new("settings_write_failed", format!("Failed to write settings: {err}."))
		})?;
		fs::rename(&tmp_path, &path).map_err(|err| {
			AppError::new("settings_write_failed", format!("Failed to save settings: {err}."))
		})?;

		Ok(())
	}

	fn settings_path(app: &tauri::AppHandle) -> Result<PathBuf, AppError> {
		let dir = app.path().app_config_dir().map_err(|err| {
			AppError::new(
				"settings_path_failed",
				format!("Failed to resolve settings path: {err}."),
			)
		})?;
		Ok(dir.join(SETTINGS_FILENAME))
	}
}

fn apply_patch_to_settings(
	settings: &mut SettingsFile,
	patch: SettingsPatch,
) -> Result<(), AppError> {
	if let Some(llm) = patch.llm {
		if let Some(base_url) = llm.base_url {
			settings.llm.base_url = base_url.trim().to_string();
		}
		if let Some(model) = llm.model {
			settings.llm.model = model.trim().to_string();
		}
		if let Some(temperature) = llm.temperature {
			settings.llm.temperature = temperature;
		}
		if let Some(system_prompt) = llm.system_prompt {
			settings.llm.system_prompt = system_prompt;
		}
		if let Some(api_key) = llm.api_key {
			settings.llm.api_key = api_key.trim().to_string();
		}
	}

	if let Some(session) = patch.session
		&& let Some(timeout_ms) = session.silence_timeout_ms
	{
		settings.session.silence_timeout_ms = timeout_ms;
	}

	if let Some(stt) = patch.stt {
		apply_stt_patch(&mut settings.stt, stt);
	}

	settings.stt.validate()?;

	Ok(())
}

fn apply_stt_patch(settings: &mut SttSettings, patch: SttSettingsPatch) {
	if let Some(sherpa) = patch.sherpa {
		if let Some(model_dir) = sherpa.model_dir {
			settings.sherpa.model_dir = model_dir.trim().to_string();
		}
		if let Some(provider) = sherpa.provider {
			settings.sherpa.provider = provider.trim().to_string();
		}
		if let Some(num_threads) = sherpa.num_threads {
			settings.sherpa.num_threads = Some(num_threads);
		}
		if let Some(decoding_method) = sherpa.decoding_method {
			settings.sherpa.decoding_method = decoding_method.trim().to_string();
		}
		if let Some(max_active_paths) = sherpa.max_active_paths {
			settings.sherpa.max_active_paths = max_active_paths;
		}
		if let Some(value) = sherpa.rule1_min_trailing_silence {
			settings.sherpa.rule1_min_trailing_silence = value;
		}
		if let Some(value) = sherpa.rule2_min_trailing_silence {
			settings.sherpa.rule2_min_trailing_silence = value;
		}
		if let Some(value) = sherpa.rule3_min_utterance_length {
			settings.sherpa.rule3_min_utterance_length = value;
		}
		if let Some(prefer_int8) = sherpa.prefer_int8 {
			settings.sherpa.prefer_int8 = prefer_int8;
		}
		if let Some(use_int8_decoder) = sherpa.use_int8_decoder {
			settings.sherpa.use_int8_decoder = use_int8_decoder;
		}
		if let Some(chunk_ms) = sherpa.chunk_ms {
			settings.sherpa.chunk_ms = chunk_ms;
		}
	}

	if let Some(whisper) = patch.whisper {
		if let Some(model_path) = whisper.model_path {
			settings.whisper.model_path = model_path.trim().to_string();
		}
		if let Some(language) = whisper.language {
			settings.whisper.language = language.trim().to_string();
		}
		if let Some(num_threads) = whisper.num_threads {
			settings.whisper.num_threads = Some(num_threads);
		}
		if let Some(force_gpu) = whisper.force_gpu {
			settings.whisper.force_gpu = Some(force_gpu);
		}
	}

	if let Some(window) = patch.window {
		if let Some(enabled) = window.enabled {
			settings.window.enabled = enabled;
		}
		if let Some(window_ms) = window.window_ms {
			settings.window.window_ms = window_ms;
		}
		if let Some(step_ms) = window.step_ms {
			settings.window.step_ms = step_ms;
		}
		if let Some(context_ms) = window.context_ms {
			settings.window.context_ms = context_ms;
		}
		if let Some(min_mean_abs) = window.min_mean_abs {
			settings.window.min_mean_abs = min_mean_abs;
		}
		if let Some(emit_every) = window.emit_every {
			settings.window.emit_every = emit_every;
		}
	}

	if let Some(merge) = patch.merge {
		if let Some(stable_ticks) = merge.stable_ticks {
			settings.merge.stable_ticks = stable_ticks;
		}
		if let Some(rollback_threshold_tokens) = merge.rollback_threshold_tokens {
			settings.merge.rollback_threshold_tokens = rollback_threshold_tokens;
		}
		if let Some(overlap_k_words) = merge.overlap_k_words {
			settings.merge.overlap_k_words = overlap_k_words;
		}
		if let Some(overlap_k_chars) = merge.overlap_k_chars {
			settings.merge.overlap_k_chars = overlap_k_chars;
		}
	}

	if let Some(profiles) = patch.profiles {
		if let Some(window_best_of) = profiles.window_best_of {
			settings.profiles.window_best_of = window_best_of;
		}
		if let Some(second_pass_best_of) = profiles.second_pass_best_of {
			settings.profiles.second_pass_best_of = second_pass_best_of;
		}
	}
}
