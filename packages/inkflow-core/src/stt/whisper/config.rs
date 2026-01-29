use std::path::PathBuf;

use whisper_rs::{WhisperContext, WhisperContextParameters};

use crate::{error::AppError, settings::WhisperSettings};

#[derive(Debug, Clone)]
pub struct WhisperConfig {
	#[allow(dead_code)]
	pub model_path: PathBuf,
	pub language: String,
	pub num_threads: Option<i32>,
	#[allow(dead_code)]
	pub force_gpu: Option<bool>,
}

#[derive(Clone, Copy, Debug)]
pub struct WhisperDecodeProfile {
	pub best_of: u8,
}

pub fn resolve_whisper_config(settings: &WhisperSettings) -> Result<WhisperConfig, AppError> {
	let model_path = resolve_existing_model_path(settings)?;
	let language = resolve_whisper_language(settings)?;
	let num_threads = resolve_whisper_threads(settings);
	let force_gpu = resolve_whisper_force_gpu(settings);

	Ok(WhisperConfig { model_path, language, num_threads, force_gpu })
}

pub fn load_whisper_context(settings: &WhisperSettings) -> Result<WhisperContext, AppError> {
	let model_path = resolve_existing_model_path(settings)?;
	let force_gpu = resolve_whisper_force_gpu(settings);

	let mut context_params = WhisperContextParameters::default();
	if let Some(force_gpu) = force_gpu {
		context_params.use_gpu = force_gpu;
	}

	WhisperContext::new_with_params(&model_path.display().to_string(), context_params).map_err(
		|err| AppError::new("whisper_init_failed", format!("Failed to load whisper model: {err}.")),
	)
}

fn resolve_existing_model_path(settings: &WhisperSettings) -> Result<PathBuf, AppError> {
	let model_path = resolve_whisper_model_path(settings);
	if !model_path.is_file() {
		return Err(AppError::new(
			"whisper_model_missing",
			format!("Whisper model file does not exist: {}.", model_path.display()),
		));
	}

	Ok(model_path)
}

fn resolve_whisper_model_path(settings: &WhisperSettings) -> PathBuf {
	const DEFAULT_MODEL_NAME: &str = "ggml-large-v3-turbo-q8_0.bin";

	if cfg!(debug_assertions)
		&& let Ok(path) = std::env::var("INKFLOW_WHISPER_MODEL_PATH")
	{
		let path = path.trim();
		if !path.is_empty() {
			return PathBuf::from(path);
		}
	}

	let configured = settings.model_path.trim();
	if !configured.is_empty() {
		return PathBuf::from(configured);
	}

	if let Ok(exe_path) = std::env::current_exe()
		&& let Some(exe_dir) = exe_path.parent()
	{
		if cfg!(target_os = "macos")
			&& let Some(contents_dir) = exe_dir.parent()
		{
			let resources_candidate = contents_dir
				.join("Resources")
				.join("models")
				.join("whisper")
				.join(DEFAULT_MODEL_NAME);
			if resources_candidate.is_file() {
				return resources_candidate;
			}
		}

		for ancestor in exe_dir.ancestors() {
			let candidate = ancestor.join("models").join("whisper").join(DEFAULT_MODEL_NAME);
			if candidate.is_file() {
				return candidate;
			}
		}
	}

	PathBuf::from("models").join("whisper").join(DEFAULT_MODEL_NAME)
}

fn resolve_whisper_language(settings: &WhisperSettings) -> Result<String, AppError> {
	if cfg!(debug_assertions)
		&& let Ok(language) = std::env::var("INKFLOW_WHISPER_LANGUAGE")
	{
		let language = language.trim().to_string();
		if !language.is_empty() {
			if language.contains('\0') {
				return Err(AppError::new(
					"whisper_config_invalid",
					"INKFLOW_WHISPER_LANGUAGE must not contain NUL bytes.",
				));
			}
			return Ok(language);
		}
	}

	let language = settings.language.trim().to_string();
	if language.contains('\0') {
		return Err(AppError::new(
			"settings_invalid",
			"Whisper language must not contain NUL bytes.",
		));
	}

	Ok(if language.is_empty() { "en".into() } else { language })
}

fn resolve_whisper_threads(settings: &WhisperSettings) -> Option<i32> {
	if cfg!(debug_assertions)
		&& let Ok(value) = std::env::var("INKFLOW_WHISPER_NUM_THREADS")
		&& let Ok(parsed) = value.trim().parse::<i32>()
		&& parsed > 0
	{
		return Some(parsed);
	}

	settings.num_threads.filter(|v| *v > 0)
}

fn resolve_whisper_force_gpu(settings: &WhisperSettings) -> Option<bool> {
	if cfg!(debug_assertions)
		&& let Ok(value) = std::env::var("INKFLOW_WHISPER_FORCE_GPU")
		&& let Some(parsed) = parse_bool(&value)
	{
		return Some(parsed);
	}

	settings.force_gpu
}

fn parse_bool(value: &str) -> Option<bool> {
	match value.trim().to_lowercase().as_str() {
		"1" | "true" | "yes" | "y" | "on" => Some(true),
		"0" | "false" | "no" | "n" | "off" => Some(false),
		_ => None,
	}
}
