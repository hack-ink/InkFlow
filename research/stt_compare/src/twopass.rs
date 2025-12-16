// std
use std::{
	ffi::OsStr,
	time::{Duration, Instant},
};
// self
use crate::{
	accuracy, audio,
	config::{CommonConfig, RunConfig},
	prelude::*,
	sherpa, util, whisper,
};
use sherpa_onnx::{OnlineRecognizer, OnlineRecognizerConfig};

pub fn run(common: &CommonConfig, run: &RunConfig) -> Result<()> {
	run_wav(common, run, &run.wav_path)
}

fn run_wav(common: &CommonConfig, run: &RunConfig, audio_path: &Path) -> Result<()> {
	let audio = audio::load_wav_mono_float32(audio_path)?;
	let audio_ms = audio.duration_ms();

	if audio_ms == 0 {
		return Err(eyre::eyre!("Audio duration must be greater than zero."));
	}

	let sherpa_files = sherpa::resolve_sherpa_model_files(
		&common.sherpa.model_path,
		common.sherpa.prefer_int8,
		common.sherpa.use_int8_decoder,
	)?;
	let sherpa_cfg = OnlineRecognizerConfig {
		tokens: sherpa_files.tokens.to_string_lossy().into_owned(),
		encoder: sherpa_files.encoder.to_string_lossy().into_owned(),
		decoder: sherpa_files.decoder.to_string_lossy().into_owned(),
		joiner: sherpa_files.joiner.to_string_lossy().into_owned(),
		provider: common.sherpa.provider.clone(),
		num_threads: common.sherpa.num_threads,
		decoding_method: common.sherpa.decoding_method.clone(),
		max_active_paths: common.sherpa.max_active_paths,
		..Default::default()
	};
	let sherpa = OnlineRecognizer::new(sherpa_cfg)
		.map_err(|err| eyre::eyre!("Failed to create sherpa-onnx recognizer: {err}"))?;
	let stream = sherpa
		.create_stream()
		.map_err(|err| eyre::eyre!("Failed to create sherpa-onnx stream: {err}"))?;
	let whisper_ctx =
		whisper::create_whisper_context(&common.whisper.model_path, common.whisper.force_gpu)?;

	println!("[config] mode=twopass input=wav");
	println!("[config] audio={} duration_ms={audio_ms}", audio_path.display());
	println!(
		"[config] sherpa model_path={} provider={} decoding_method={} max_active_paths={} threads={}",
		common.sherpa.model_path.display(),
		common.sherpa.provider,
		common.sherpa.decoding_method,
		common.sherpa.max_active_paths,
		common.sherpa.num_threads
	);
	println!(
		"[config] sherpa files encoder={} joiner={} decoder={}",
		sherpa_files.encoder.file_name().and_then(OsStr::to_str).unwrap_or("<unknown>"),
		sherpa_files.joiner.file_name().and_then(OsStr::to_str).unwrap_or("<unknown>"),
		sherpa_files.decoder.file_name().and_then(OsStr::to_str).unwrap_or("<unknown>")
	);
	println!(
		"[config] whisper model={} gpu={} threads={} lang={}",
		common.whisper.model_path.display(),
		match common.whisper.force_gpu {
			Some(true) => "forced_on",
			Some(false) => "forced_off",
			None => "default",
		},
		common.whisper.num_threads.map(|v| v.to_string()).unwrap_or_else(|| "default".to_string()),
		&common.whisper.language
	);
	println!(
		"[config] sherpa_chunk_ms={} print_partials={} max_text_len={}",
		common.sherpa_chunk_ms, common.print_partials, common.max_text_len
	);

	if let Some(reference) = run.reference_text.as_deref() {
		println!("[ref] {reference}");
	}

	println!();

	let chunk_size = audio::chunk_size_samples(audio.sample_rate_hz, common.sherpa_chunk_ms)?;
	let mut segment_buffer = <Vec<f32>>::new();
	let mut segment_start_samples = 0_usize;
	let mut processed_samples = 0_usize;
	let mut segment_index = 0_u32;
	let mut last_partial = String::new();
	let mut sherpa_decode_total = Duration::ZERO;
	let mut whisper_decode_total = Duration::ZERO;
	let mut sherpa_final_parts = <Vec<String>>::new();
	let mut whisper_final_parts = <Vec<String>>::new();

	for chunk in audio.samples.chunks(chunk_size) {
		stream.accept_waveform(audio.sample_rate_hz as i32, chunk);
		segment_buffer.extend_from_slice(chunk);
		processed_samples = processed_samples.saturating_add(chunk.len());

		let sherpa_decode_start = Instant::now();

		sherpa.decode(&stream);

		sherpa_decode_total += sherpa_decode_start.elapsed();

		let result = sherpa
			.result_json(&stream)
			.map_err(|err| eyre::eyre!("Failed to read sherpa-onnx result: {err}"))?;

		if common.print_partials {
			let text = result.text.trim();

			if !text.is_empty() && text != last_partial {
				let t_ms = audio::samples_to_ms(processed_samples, audio.sample_rate_hz);

				println!(
					"[partial] t_ms={t_ms} engine=sherpa text={}",
					util::truncate_text(text, common.max_text_len)
				);

				last_partial = text.to_string();
			}
		}

		if stream.is_endpoint() {
			segment_index = segment_index.saturating_add(1);

			let segment_end_samples = processed_samples;
			let segment_duration_ms = audio::samples_to_ms(
				segment_end_samples.saturating_sub(segment_start_samples),
				audio.sample_rate_hz,
			);
			let sherpa_text = result.text.trim().to_string();
			let whisper_audio_16k =
				audio::resample_linear_to_16k(&segment_buffer, audio.sample_rate_hz);
			let whisper_decode_start = Instant::now();
			let whisper_text = whisper::transcribe(
				&whisper_ctx,
				&whisper_audio_16k,
				&common.whisper,
				segment_index,
			)?;
			let whisper_elapsed = whisper_decode_start.elapsed();

			whisper_decode_total += whisper_elapsed;

			let segment_start_ms =
				audio::samples_to_ms(segment_start_samples, audio.sample_rate_hz);
			let segment_end_ms = audio::samples_to_ms(segment_end_samples, audio.sample_rate_hz);
			let whisper_latency_ms = whisper_elapsed.as_secs_f64() * 1000.0_f64;

			println!(
				"[seg] idx={} start_ms={} end_ms={} dur_ms={} whisper_latency_ms={:.0} whisper_rtf={:.4}",
				segment_index,
				segment_start_ms,
				segment_end_ms,
				segment_duration_ms,
				whisper_latency_ms,
				util::safe_ratio(whisper_latency_ms, segment_duration_ms as f64)
			);
			println!("  [sherpa] {}", sherpa_text);
			println!("  [whisper] {}", whisper_text);
			println!();

			if !sherpa_text.trim().is_empty() {
				sherpa_final_parts.push(sherpa_text);
			}
			if !whisper_text.trim().is_empty() {
				whisper_final_parts.push(whisper_text);
			}

			segment_buffer.clear();
			segment_start_samples = segment_end_samples;
			last_partial.clear();
			stream.reset();
		}
	}

	stream.input_finished();

	let sherpa_decode_start = Instant::now();

	sherpa.decode(&stream);
	sherpa_decode_total += sherpa_decode_start.elapsed();

	let result = sherpa
		.result_json(&stream)
		.map_err(|err| eyre::eyre!("Failed to read sherpa-onnx result: {err}"))?;

	if !segment_buffer.is_empty() {
		segment_index = segment_index.saturating_add(1);

		let segment_end_samples = processed_samples;
		let segment_duration_ms = audio::samples_to_ms(
			segment_end_samples.saturating_sub(segment_start_samples),
			audio.sample_rate_hz,
		);
		let sherpa_text = result.text.trim().to_string();
		let whisper_audio_16k =
			audio::resample_linear_to_16k(&segment_buffer, audio.sample_rate_hz);
		let whisper_decode_start = Instant::now();
		let whisper_text =
			whisper::transcribe(&whisper_ctx, &whisper_audio_16k, &common.whisper, segment_index)?;
		let whisper_elapsed = whisper_decode_start.elapsed();

		whisper_decode_total += whisper_elapsed;

		let segment_start_ms = audio::samples_to_ms(segment_start_samples, audio.sample_rate_hz);
		let segment_end_ms = audio::samples_to_ms(segment_end_samples, audio.sample_rate_hz);
		let whisper_latency_ms = whisper_elapsed.as_secs_f64() * 1000.0_f64;

		println!(
			"[seg] idx={} start_ms={} end_ms={} dur_ms={} whisper_latency_ms={:.0} whisper_rtf={:.4}",
			segment_index,
			segment_start_ms,
			segment_end_ms,
			segment_duration_ms,
			whisper_latency_ms,
			util::safe_ratio(whisper_latency_ms, segment_duration_ms as f64)
		);
		println!("  [sherpa] {}", sherpa_text);
		println!("  [whisper] {}", whisper_text);
		println!();

		if !sherpa_text.trim().is_empty() {
			sherpa_final_parts.push(sherpa_text);
		}
		if !whisper_text.trim().is_empty() {
			whisper_final_parts.push(whisper_text);
		}
	}

	let sherpa_final = util::join_text_parts(&sherpa_final_parts);
	let whisper_final = util::join_text_parts(&whisper_final_parts);

	if !sherpa_final.trim().is_empty() {
		println!("[final] engine=sherpa text={}", sherpa_final);
	}
	if !whisper_final.trim().is_empty() {
		println!("[final] engine=whisper_twopass text={}", whisper_final);
	}

	if let Some(reference) = run.reference_text.as_deref()
		&& !reference.trim().is_empty()
	{
		if !sherpa_final.trim().is_empty() {
			accuracy::print_accuracy("sherpa", reference, &sherpa_final);
		}
		if !whisper_final.trim().is_empty() {
			accuracy::print_accuracy("whisper_twopass", reference, &whisper_final);
		}
	}

	println!("[summary]");

	let audio_ms_f64 = audio_ms as f64;
	let sherpa_decode_ms = sherpa_decode_total.as_secs_f64() * 1000.0_f64;
	let whisper_decode_ms = whisper_decode_total.as_secs_f64() * 1000.0_f64;

	println!(
		"  sherpa_decode_ms={:.0} sherpa_rtf={:.4}",
		sherpa_decode_ms,
		util::safe_ratio(sherpa_decode_ms, audio_ms_f64)
	);
	println!(
		"  whisper_decode_ms={:.0} whisper_rtf={:.4}",
		whisper_decode_ms,
		util::safe_ratio(whisper_decode_ms, audio_ms_f64)
	);
	println!("  segments={segment_index}");

	Ok(())
}
