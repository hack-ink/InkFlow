// crates.io
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};
// self
use crate::{config::WhisperConfig, prelude::*};

pub fn create_whisper_context(
	model_path: &Path,
	force_gpu: Option<bool>,
) -> Result<WhisperContext> {
	if !model_path.is_file() {
		return Err(eyre::eyre!("Whisper model file not found: {}.", model_path.display()));
	}

	let mut context_params = WhisperContextParameters::default();

	if let Some(force_gpu) = force_gpu {
		context_params.use_gpu = force_gpu;
	}

	WhisperContext::new_with_params(&model_path.display().to_string(), context_params)
		.map_err(|e| eyre::eyre!("Failed to load whisper model: {e}"))
}

pub fn transcribe(
	ctx: &WhisperContext,
	audio_16k: &[f32],
	config: &WhisperConfig,
	run_id: u32,
) -> Result<String> {
	if audio_16k.is_empty() {
		return Ok(String::new());
	}

	let mut state = ctx
		.create_state()
		.map_err(|e| eyre::eyre!("Failed to create whisper decoder state: {e}"))?;
	let sampling = if let Some(beam_size) = config.beam_size {
		SamplingStrategy::BeamSearch { beam_size, patience: config.beam_patience }
	} else {
		SamplingStrategy::Greedy { best_of: config.best_of }
	};
	let mut params = FullParams::new(sampling);

	if let Some(threads) = config.num_threads {
		params.set_n_threads(threads);
	}

	let language = config.language.as_str();

	if language.contains('\0') {
		return Err(eyre::eyre!("Language must not contain NUL bytes."));
	}
	if language == "auto" {
		let detected = detect_whisper_language(ctx, audio_16k, config)?;
		params.set_detect_language(false);
		params.set_language(Some(detected));
	} else {
		params.set_detect_language(false);
		params.set_language(Some(language));
	}

	params.set_translate(false);
	params.set_print_special(false);
	params.set_print_progress(false);
	params.set_print_realtime(false);
	params.set_print_timestamps(false);
	state
		.full(params, audio_16k)
		.map_err(|e| eyre::eyre!("Whisper transcription failed (run {run_id}): {e}"))?;

	let mut text = String::new();

	for segment in state.as_iter() {
		let segment_text = segment.to_string();
		let trimmed = segment_text.trim();

		if trimmed.is_empty() {
			continue;
		}
		if !text.is_empty() {
			text.push(' ');
		}

		text.push_str(trimmed);
	}

	Ok(text)
}

fn detect_whisper_language(
	ctx: &WhisperContext,
	audio_16k: &[f32],
	config: &WhisperConfig,
) -> Result<&'static str> {
	let mut state = ctx
		.create_state()
		.map_err(|e| eyre::eyre!("Failed to create whisper state for language detection: {e}"))?;
	let threads = whisper_threads_usize(config.num_threads);

	state
		.pcm_to_mel(audio_16k, threads)
		.map_err(|e| eyre::eyre!("Whisper PCM-to-mel failed during language detection: {e}"))?;

	let (lang_id, _) = state
		.lang_detect(0, threads)
		.map_err(|e| eyre::eyre!("Whisper language detection failed: {e}"))?;

	whisper_rs::get_lang_str(lang_id)
		.ok_or_else(|| eyre::eyre!("Whisper returned an unknown language id: {lang_id}."))
}

fn whisper_threads_usize(threads: Option<i32>) -> usize {
	match threads {
		Some(v) if v > 0 => usize::try_from(v).unwrap_or(1),
		_ => 1,
	}
}
