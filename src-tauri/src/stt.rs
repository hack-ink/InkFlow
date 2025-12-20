mod whisper;

use std::path::{Path, PathBuf};

use crate::{error::AppError, settings::SherpaSettings};

pub use whisper::{
	WhisperConfig, WhisperDecodeProfile, WhisperDecodeResult, WhisperDecodedSegment,
	load_whisper_context, resample_linear_to_16k, resolve_whisper_config, transcribe,
	transcribe_segments,
};

#[derive(Clone, Debug)]
pub(crate) struct WindowJobSnapshot {
	pub engine_generation: u64,
	pub window_generation: u64,
	pub job_id: u64,
	pub window_end_16k_samples: u64,
	pub window_len_16k_samples: usize,
	pub context_len_16k_samples: usize,
}

pub fn resolve_sherpa_config(
	settings: &SherpaSettings,
) -> Result<sherpa_onnx::OnlineRecognizerConfig, AppError> {
	let model_dir = resolve_sherpa_model_dir(settings);
	let tokens = model_dir.join("tokens.txt");

	let encoder = pick_model_file(
		&model_dir,
		"encoder-epoch-99-avg-1.int8.onnx",
		"encoder-epoch-99-avg-1.onnx",
		settings.prefer_int8,
	)?;

	let joiner = pick_model_file(
		&model_dir,
		"joiner-epoch-99-avg-1.int8.onnx",
		"joiner-epoch-99-avg-1.onnx",
		settings.prefer_int8,
	)?;

	let decoder = if settings.use_int8_decoder {
		pick_model_file(
			&model_dir,
			"decoder-epoch-99-avg-1.int8.onnx",
			"decoder-epoch-99-avg-1.onnx",
			true,
		)?
	} else {
		pick_model_file(
			&model_dir,
			"decoder-epoch-99-avg-1.onnx",
			"decoder-epoch-99-avg-1.int8.onnx",
			true,
		)?
	};

	for required in [&tokens, &encoder, &decoder, &joiner] {
		if !required.is_file() {
			return Err(AppError::new(
				"stt_model_missing",
				format!("Required model file does not exist: {}.", required.display()),
			));
		}
	}

	let default_threads = std::thread::available_parallelism()
		.map(|n| (n.get().saturating_div(2)).max(1) as i32)
		.unwrap_or(2);
	let num_threads = settings.num_threads.unwrap_or(default_threads);

	Ok(sherpa_onnx::OnlineRecognizerConfig {
		tokens: tokens.display().to_string(),
		encoder: encoder.display().to_string(),
		decoder: decoder.display().to_string(),
		joiner: joiner.display().to_string(),
		provider: settings.provider.clone(),
		num_threads,
		decoding_method: settings.decoding_method.clone(),
		max_active_paths: settings.max_active_paths,
		rule1_min_trailing_silence: settings.rule1_min_trailing_silence,
		rule2_min_trailing_silence: settings.rule2_min_trailing_silence,
		rule3_min_utterance_length: settings.rule3_min_utterance_length,
		..Default::default()
	})
}

fn resolve_sherpa_model_dir(settings: &SherpaSettings) -> PathBuf {
	const DEFAULT_MODEL_DIR_NAME: &str = "sherpa-onnx-streaming-zipformer-en-2023-06-21";

	if cfg!(debug_assertions)
		&& let Ok(path) = std::env::var("AIR_SHERPA_ONNX_MODEL_DIR")
	{
		let path = path.trim();
		if !path.is_empty() {
			return PathBuf::from(path);
		}
	}

	let configured = settings.model_dir.trim();
	if !configured.is_empty() {
		return PathBuf::from(configured);
	}

	if let Ok(exe_path) = std::env::current_exe()
		&& let Some(exe_dir) = exe_path.parent()
	{
		if cfg!(target_os = "macos")
			&& let Some(contents_dir) = exe_dir.parent()
		{
			let resources_candidate =
				contents_dir.join("Resources").join("model").join(DEFAULT_MODEL_DIR_NAME);
			if resources_candidate.is_dir() {
				return resources_candidate;
			}
		}

		for ancestor in exe_dir.ancestors() {
			let candidate = ancestor.join("model").join(DEFAULT_MODEL_DIR_NAME);
			if candidate.is_dir() {
				return candidate;
			}
		}
	}

	PathBuf::from("model").join(DEFAULT_MODEL_DIR_NAME)
}

fn pick_model_file(
	model_dir: &Path,
	primary: &str,
	fallback: &str,
	primary_first: bool,
) -> Result<PathBuf, AppError> {
	let p1 = model_dir.join(primary);
	let p2 = model_dir.join(fallback);

	if primary_first {
		if p1.is_file() {
			return Ok(p1);
		}
		if p2.is_file() {
			return Ok(p2);
		}
	} else {
		if p2.is_file() {
			return Ok(p2);
		}
		if p1.is_file() {
			return Ok(p1);
		}
	}

	Err(AppError::new(
		"stt_model_missing",
		format!("Missing model file. Expected one of {} or {}.", p1.display(), p2.display()),
	))
}
