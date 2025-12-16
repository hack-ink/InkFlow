// std
use std::time::{Duration, Instant};
// self
use crate::{
	accuracy, audio,
	config::{CommonConfig, REQUIRED_SAMPLE_RATE_HZ, RunConfig},
	prelude::*,
	util, whisper,
};

pub fn run(common: &CommonConfig, run: &RunConfig) -> Result<()> {
	run_wav(common, run, &run.wav_path)
}

fn run_wav(common: &CommonConfig, run: &RunConfig, audio_path: &Path) -> Result<()> {
	let audio = audio::load_wav_mono_float32(audio_path)?;
	let audio_ms = audio.duration_ms();

	if audio_ms == 0 {
		return Err(eyre::eyre!("Audio duration must be greater than zero."));
	}

	let whisper_ctx =
		whisper::create_whisper_context(&common.whisper.model_path, common.whisper.force_gpu)?;
	let audio_16k = audio::resample_linear_to_16k(&audio.samples, audio.sample_rate_hz);
	let duration_16k_ms = audio::samples_to_ms(audio_16k.len(), REQUIRED_SAMPLE_RATE_HZ).max(1);

	if duration_16k_ms == 0 {
		return Err(eyre::eyre!("Resampled audio duration must be greater than zero."));
	}

	let window_samples =
		audio::chunk_size_samples(REQUIRED_SAMPLE_RATE_HZ, common.whisper.window_ms.max(100))?;
	let step_samples = audio::chunk_size_samples(REQUIRED_SAMPLE_RATE_HZ, common.whisper.step_ms)?;

	if step_samples == 0 {
		return Err(eyre::eyre!("--whisper-step-ms must be greater than zero."));
	}

	println!("[config] mode=whisper-window input=wav");
	println!("[config] audio={} duration_ms={audio_ms}", audio_path.display());
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
		"[config] window_ms={} step_ms={} print_partials={} whisper_tick_every={} max_text_len={}",
		common.whisper.window_ms,
		common.whisper.step_ms,
		common.print_partials,
		common.whisper_tick_every,
		common.max_text_len
	);

	if let Some(reference) = run.reference_text.as_deref() {
		println!("[ref] {reference}");
	}

	println!();

	let mut tick_index: u32 = 0;
	let mut decode_times = <Vec<Duration>>::new();
	let mut last_partial = String::new();
	let step_ms = common.whisper.step_ms as f64;
	let mut end = step_samples;

	while end <= audio_16k.len() {
		tick_index = tick_index.saturating_add(1);
		let start = end.saturating_sub(window_samples);
		let slice = &audio_16k[start..end];
		let decode_start = Instant::now();
		let text = whisper::transcribe(&whisper_ctx, slice, &common.whisper, tick_index)?;
		let elapsed = decode_start.elapsed();

		decode_times.push(elapsed);

		let decode_ms = elapsed.as_secs_f64() * 1_000.0_f64;
		let t_ms = audio::samples_to_ms(end, REQUIRED_SAMPLE_RATE_HZ);
		let window_ms = audio::samples_to_ms(slice.len(), REQUIRED_SAMPLE_RATE_HZ).max(1);
		let ratio = util::safe_ratio(decode_ms, step_ms);
		let should_print = if common.print_partials {
			text != last_partial
		} else {
			common.whisper_tick_every > 0
				&& (tick_index == 1 || tick_index.is_multiple_of(common.whisper_tick_every))
		};

		if should_print {
			println!(
				"[tick] idx={} t_ms={} window_ms={} decode_ms={:.0} ratio={:.2}",
				tick_index, t_ms, window_ms, decode_ms, ratio
			);
			println!("  [whisper] {}", util::truncate_text(&text, common.max_text_len));
		}
		if common.print_partials {
			last_partial = text.clone();
		}

		end = end.saturating_add(step_samples);
	}

	let final_decode_start = Instant::now();
	let final_text = whisper::transcribe(
		&whisper_ctx,
		&audio_16k,
		&common.whisper,
		tick_index.saturating_add(1),
	)?;
	let final_elapsed = final_decode_start.elapsed();

	println!("[final] engine=whisper text={}", util::truncate_text(&final_text, usize::MAX));

	if let Some(reference) = run.reference_text.as_deref()
		&& !reference.trim().is_empty()
	{
		accuracy::print_accuracy("whisper", reference, &final_text);
	}

	println!();

	let stats = audio::summarize_durations(&decode_times);

	println!("[summary]");
	println!("  ticks={tick_index}");

	let duration_16k_ms_f64 = duration_16k_ms as f64;
	let final_decode_ms = final_elapsed.as_secs_f64() * 1_000.0_f64;

	println!(
		"  tick_mean_decode_ms={:.0} tick_mean_ratio={:.2} tick_max_decode_ms={:.0} tick_max_ratio={:.2}",
		stats.mean_ms,
		util::safe_ratio(stats.mean_ms, step_ms),
		stats.max_ms,
		util::safe_ratio(stats.max_ms, step_ms)
	);
	println!(
		"  final_decode_ms={:.0} final_rtf={:.4}",
		final_decode_ms,
		util::safe_ratio(final_decode_ms, duration_16k_ms_f64)
	);

	Ok(())
}
