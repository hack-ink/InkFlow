use whisper_rs::{FullParams, SamplingStrategy, WhisperContext};

use crate::error::AppError;

use super::config::{WhisperConfig, WhisperDecodeProfile};
use super::text::append_trimmed_text;

#[derive(Clone, Debug)]
pub struct WhisperDecodedSegment {
	#[allow(dead_code)]
	pub t0_ms: u64,
	#[allow(dead_code)]
	pub t1_ms: u64,
	pub text: String,
}

#[derive(Clone, Debug)]
pub struct WhisperDecodeResult {
	pub text: String,
	pub segments: Vec<WhisperDecodedSegment>,
	pub has_timestamps: bool,
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
		params.set_detect_language(false);
		params.set_language(None);
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

fn whisper_ts_to_ms(ts: i64) -> u64 {
	if ts <= 0 {
		return 0;
	}

	(ts as u64).saturating_mul(10)
}
