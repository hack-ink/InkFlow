// std
use std::time::Duration;
// crates.io
use hound::WavReader;
// self
use crate::{config::REQUIRED_SAMPLE_RATE_HZ, prelude::*};

pub struct AudioInput {
	pub samples: Vec<f32>,
	pub sample_rate_hz: u32,
}
impl AudioInput {
	pub fn duration_ms(&self) -> u64 {
		if self.samples.is_empty() {
			return 0;
		}
		samples_to_ms(self.samples.len(), self.sample_rate_hz).max(1)
	}
}

pub struct DurationSummary {
	pub max_ms: f64,
	pub mean_ms: f64,
}

pub fn load_wav_mono_float32(path: &Path) -> Result<AudioInput> {
	let reader = WavReader::open(path)
		.wrap_err_with(|| format!("Failed to open WAV: {}.", path.display()))?;
	let spec = reader.spec();

	if spec.bits_per_sample != 16 {
		return Err(eyre::eyre!("WAV bits per sample must be 16, got {}.", spec.bits_per_sample));
	}
	if spec.sample_format != hound::SampleFormat::Int {
		return Err(eyre::eyre!(
			"WAV sample format must be integer PCM, got {:?}.",
			spec.sample_format
		));
	}
	if spec.channels != 1 && spec.channels != 2 {
		return Err(eyre::eyre!(
			"WAV channel count must be 1 (mono) or 2 (stereo), got {}.",
			spec.channels
		));
	}
	if spec.sample_rate == 0 {
		return Err(eyre::eyre!("WAV sample rate must be greater than zero."));
	}

	let samples_i16 = reader
		.into_samples::<i16>()
		.collect::<Result<Vec<_>, _>>()
		.wrap_err_with(|| format!("Failed to read WAV samples: {}.", path.display()))?;
	let mut audio_f32 = vec![0.0_f32; samples_i16.len()];

	whisper_rs::convert_integer_to_float_audio(&samples_i16, &mut audio_f32)
		.map_err(|err| eyre::eyre!("Failed to convert PCM samples to float: {err}"))?;

	let samples = if spec.channels == 1 {
		audio_f32
	} else {
		whisper_rs::convert_stereo_to_mono_audio(&audio_f32)
			.map_err(|err| eyre::eyre!("Failed to convert stereo to mono: {err}"))?
	};

	Ok(AudioInput { samples, sample_rate_hz: spec.sample_rate })
}

pub fn samples_to_ms(samples: usize, sample_rate_hz: u32) -> u64 {
	if sample_rate_hz == 0 {
		return 0;
	}
	(samples as u64).saturating_mul(1000).saturating_div(sample_rate_hz as u64)
}

pub fn resample_linear_to_16k(input: &[f32], input_sample_rate_hz: u32) -> Vec<f32> {
	if input.is_empty() {
		return Vec::new();
	}
	if input_sample_rate_hz == REQUIRED_SAMPLE_RATE_HZ {
		return input.to_vec();
	}
	if input_sample_rate_hz == 0 {
		return Vec::new();
	}

	let ratio = REQUIRED_SAMPLE_RATE_HZ as f64 / input_sample_rate_hz as f64;
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

pub fn summarize_durations(durations: &[Duration]) -> DurationSummary {
	let mut max_ms = 0.0_f64;
	let mut sum_ms = 0.0_f64;

	for duration in durations {
		let ms = duration.as_secs_f64() * 1000.0_f64;

		max_ms = max_ms.max(ms);
		sum_ms += ms;
	}

	let mean_ms = if durations.is_empty() { 0.0 } else { sum_ms / durations.len() as f64 };

	DurationSummary { max_ms, mean_ms }
}

pub fn chunk_size_samples(sample_rate_hz: u32, chunk_ms: u32) -> Result<usize> {
	if sample_rate_hz == 0 {
		return Err(eyre::eyre!("Sample rate must be greater than zero."));
	}
	if chunk_ms == 0 {
		return Err(eyre::eyre!("Chunk size must be greater than zero."));
	}

	let samples = (sample_rate_hz as u64).saturating_mul(chunk_ms as u64).saturating_div(1000);

	Ok(samples.max(1) as usize)
}
