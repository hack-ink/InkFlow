use std::path::PathBuf;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

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

#[derive(Clone, Debug)]
pub struct WhisperDecodedSegment {
	#[allow(dead_code)]
	pub t0_ms: u64,
	pub t1_ms: u64,
	pub text: String,
}

#[derive(Clone, Debug)]
pub struct WhisperDecodeResult {
	pub text: String,
	pub segments: Vec<WhisperDecodedSegment>,
	pub has_timestamps: bool,
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

pub fn transcribe(
	ctx: &WhisperContext,
	audio_16k: &[f32],
	config: &WhisperConfig,
	profile: WhisperDecodeProfile,
) -> Result<String, AppError> {
	if audio_16k.is_empty() {
		return Ok(String::new());
	}

	let mut state = ctx.create_state().map_err(|err| {
		AppError::new(
			"whisper_decode_failed",
			format!("Failed to create a whisper decoder state: {err}."),
		)
	})?;

	let params = build_decode_params(config, profile);

	state.full(params, audio_16k).map_err(|err| {
		AppError::new("whisper_decode_failed", format!("Whisper transcription failed: {err}."))
	})?;

	let mut text = String::new();
	for segment in state.as_iter() {
		let segment_text = segment.to_string();
		let trimmed = segment_text.trim();
		append_trimmed_text(&mut text, trimmed);
	}

	Ok(text)
}

pub fn transcribe_segments(
	ctx: &WhisperContext,
	audio_16k: &[f32],
	config: &WhisperConfig,
	profile: WhisperDecodeProfile,
) -> Result<WhisperDecodeResult, AppError> {
	if audio_16k.is_empty() {
		return Ok(WhisperDecodeResult {
			text: String::new(),
			segments: Vec::new(),
			has_timestamps: true,
		});
	}

	let mut state = ctx.create_state().map_err(|err| {
		AppError::new(
			"whisper_decode_failed",
			format!("Failed to create a whisper decoder state: {err}."),
		)
	})?;

	let params = build_decode_params(config, profile);

	state.full(params, audio_16k).map_err(|err| {
		AppError::new("whisper_decode_failed", format!("Whisper transcription failed: {err}."))
	})?;

	let n = state.full_n_segments();

	let mut text = String::new();
	let mut segments = Vec::with_capacity(n as usize);
	let has_timestamps = true;

	for i in 0..n {
		let Some(segment) = state.get_segment(i) else {
			continue;
		};

		let seg_text = segment.to_str_lossy().map_err(|err| {
			AppError::new(
				"whisper_decode_failed",
				format!("Failed to read whisper segment text: {err}."),
			)
		})?;
		let trimmed = seg_text.trim();
		if trimmed.is_empty() {
			continue;
		}

		let t0_ms = whisper_ts_to_ms(segment.start_timestamp());
		let t1_ms = whisper_ts_to_ms(segment.end_timestamp());

		append_trimmed_text(&mut text, trimmed);

		segments.push(WhisperDecodedSegment { t0_ms, t1_ms, text: trimmed.to_string() });
	}

	Ok(WhisperDecodeResult { text, segments, has_timestamps })
}

pub fn resample_linear_to_16k(input: &[f32], input_sample_rate_hz: u32) -> Vec<f32> {
	const OUTPUT_SAMPLE_RATE_HZ: u32 = 16_000;

	if input.is_empty() {
		return Vec::new();
	}
	if input_sample_rate_hz == OUTPUT_SAMPLE_RATE_HZ {
		return input.to_vec();
	}
	if input_sample_rate_hz == 0 {
		return Vec::new();
	}

	let ratio = OUTPUT_SAMPLE_RATE_HZ as f64 / input_sample_rate_hz as f64;
	let input_len = input.len();
	let output_len = if input_len <= 1 {
		1
	} else {
		(((input_len - 1) as f64) * ratio).floor().max(0.0) as usize + 1
	};
	let mut out = Vec::with_capacity(output_len);

	for i in 0..output_len {
		let src_pos = i as f64 / ratio;
		let idx = src_pos.floor() as usize;
		let frac = src_pos - idx as f64;
		let s0 = input.get(idx).copied().unwrap_or(0.0_f32) as f64;
		let s1 = input.get(idx + 1).copied().unwrap_or(s0 as f32) as f64;

		out.push((s0 + (s1 - s0) * frac) as f32);
	}

	out
}

fn append_trimmed_text(out: &mut String, trimmed: &str) {
	if trimmed.is_empty() {
		return;
	}

	if !out.is_empty() {
		out.push(' ');
	}

	out.push_str(trimmed);
}

fn build_decode_params(
	config: &WhisperConfig,
	profile: WhisperDecodeProfile,
) -> FullParams<'_, '_> {
	let best_of = profile.best_of.saturating_sub(1) as i32;
	let mut params = FullParams::new(SamplingStrategy::Greedy { best_of });
	if let Some(threads) = config.num_threads {
		params.set_n_threads(threads);
	}

	let language = config.language.as_str();
	if language == "auto" {
		params.set_detect_language(true);
	} else {
		params.set_detect_language(false);
		params.set_language(Some(language));
	}

	params.set_translate(false);

	params.set_print_special(false);
	params.set_print_progress(false);
	params.set_print_realtime(false);
	params.set_print_timestamps(false);

	params
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
				.join("model")
				.join("whisper")
				.join(DEFAULT_MODEL_NAME);
			if resources_candidate.is_file() {
				return resources_candidate;
			}
		}

		for ancestor in exe_dir.ancestors() {
			let candidate = ancestor.join("model").join("whisper").join(DEFAULT_MODEL_NAME);
			if candidate.is_file() {
				return candidate;
			}
		}
	}

	PathBuf::from("model").join("whisper").join(DEFAULT_MODEL_NAME)
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

fn whisper_ts_to_ms(ts: i64) -> u64 {
	if ts <= 0 {
		return 0;
	}

	(ts as u64).saturating_mul(10)
}
