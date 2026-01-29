use std::{
	collections::hash_map::DefaultHasher,
	hash::{Hash, Hasher},
	sync::atomic::{AtomicU64, Ordering},
	time::Instant,
};

use crate::{settings::WhisperWindowSettings, stt};

#[derive(Debug)]
pub(crate) enum WhisperJob {
	SecondPass { segment_id: u64, sample_rate_hz: u32, samples: Vec<f32>, peak_mean_abs: f32 },
	Window { snapshot: stt::WindowJobSnapshot, audio_16k: Vec<f32> },
}

pub(crate) struct SpeechActivity {
	start: Instant,
	last_ms: AtomicU64,
}

impl SpeechActivity {
	pub(crate) fn new() -> Self {
		Self { start: Instant::now(), last_ms: AtomicU64::new(0) }
	}

	pub(crate) fn mark(&self) {
		let now_ms = self.start.elapsed().as_millis() as u64;
		self.last_ms.store(now_ms, Ordering::Relaxed);
	}

	pub(crate) fn is_recent(&self, window_activity_ms: u64) -> bool {
		let last = self.last_ms.load(Ordering::Relaxed);
		if last == 0 {
			return false;
		}

		let now_ms = self.start.elapsed().as_millis() as u64;
		let hold_ms = window_activity_ms.saturating_mul(3).max(200);
		now_ms.saturating_sub(last) <= hold_ms
	}
}

pub(crate) fn ms_to_samples(sample_rate_hz: u32, ms: u64) -> usize {
	if sample_rate_hz == 0 || ms == 0 {
		return 0;
	}

	(sample_rate_hz as u64).saturating_mul(ms).saturating_div(1_000) as usize
}

pub(crate) fn audio_hash(samples: &[f32]) -> u64 {
	let mut hasher = DefaultHasher::new();
	let len = samples.len();
	let stride = (len / 128).max(1);
	for sample in samples.iter().step_by(stride) {
		sample.to_bits().hash(&mut hasher);
	}
	len.hash(&mut hasher);
	hasher.finish()
}

pub(crate) struct ActivityGate {
	pub(crate) min_mean_abs: f32,
	pub(crate) min_rms: f32,
	pub(crate) max_zero_crossing_rate: f32,
	pub(crate) min_band_energy_ratio: f32,
}

impl ActivityGate {
	pub(crate) fn new(settings: &WhisperWindowSettings) -> Self {
		Self {
			min_mean_abs: settings.min_mean_abs,
			min_rms: settings.min_rms,
			max_zero_crossing_rate: settings.max_zero_crossing_rate,
			min_band_energy_ratio: settings.min_band_energy_ratio,
		}
	}

	pub(crate) fn allows(&self, metrics: &ActivityMetrics) -> bool {
		let noise_ok = metrics.zero_crossing_rate <= self.max_zero_crossing_rate;
		if !noise_ok {
			return false;
		}

		let energy_ok = metrics.mean_abs >= self.min_mean_abs && metrics.rms >= self.min_rms;
		let energy_relaxed =
			metrics.mean_abs >= self.min_mean_abs * 0.6 && metrics.rms >= self.min_rms * 0.6;
		if !energy_ok && !energy_relaxed {
			return false;
		}

		if self.min_band_energy_ratio <= 0.0 {
			return true;
		}

		const ENERGY_BOOST: f32 = 1.5;
		const ZCR_RELAX_FACTOR: f32 = 0.8;

		let mean_boost =
			self.min_mean_abs > 0.0 && metrics.mean_abs >= self.min_mean_abs * ENERGY_BOOST;
		let rms_boost = self.min_rms > 0.0 && metrics.rms >= self.min_rms * ENERGY_BOOST;
		let band_ok = metrics.band_energy_ratio >= self.min_band_energy_ratio;
		let low_noise = metrics.zero_crossing_rate <= self.max_zero_crossing_rate * ZCR_RELAX_FACTOR;

		band_ok || (low_noise && energy_relaxed) || mean_boost || rms_boost
	}
}

pub(crate) struct ActivityMetrics {
	pub(crate) mean_abs: f32,
	pub(crate) rms: f32,
	pub(crate) zero_crossing_rate: f32,
	pub(crate) band_energy_ratio: f32,
}

pub(crate) fn activity_metrics(samples: &[f32], sample_rate_hz: u32) -> ActivityMetrics {
	if samples.is_empty() || sample_rate_hz == 0 {
		return ActivityMetrics {
			mean_abs: 0.0,
			rms: 0.0,
			zero_crossing_rate: 0.0,
			band_energy_ratio: 0.0,
		};
	}

	let mut sum_abs = 0.0_f32;
	let mut sum_sq = 0.0_f32;
	let mut zero_crossings = 0usize;
	let mut prev = samples[0];
	for &sample in samples {
		sum_abs += sample.abs();
		sum_sq += sample * sample;
		if (sample >= 0.0 && prev < 0.0) || (sample < 0.0 && prev >= 0.0) {
			zero_crossings += 1;
		}
		prev = sample;
	}

	let len = samples.len() as f32;
	let mean_abs = sum_abs / len;
	let rms = (sum_sq / len).sqrt();
	let denom = samples.len().saturating_sub(1).max(1) as f32;
	let zero_crossing_rate = zero_crossings as f32 / denom;
	let band_energy_ratio = band_energy_ratio(samples, sample_rate_hz);

	ActivityMetrics { mean_abs, rms, zero_crossing_rate, band_energy_ratio }
}

pub(crate) fn band_energy_ratio(samples: &[f32], sample_rate_hz: u32) -> f32 {
	use std::f32::consts::PI;

	if samples.is_empty() || sample_rate_hz == 0 {
		return 0.0;
	}

	let dt = 1.0_f32 / sample_rate_hz as f32;
	let hp_rc = 1.0_f32 / (2.0_f32 * PI * 300.0_f32);
	let lp_rc = 1.0_f32 / (2.0_f32 * PI * 3400.0_f32);
	let hp_alpha = hp_rc / (hp_rc + dt);
	let lp_alpha = dt / (lp_rc + dt);

	let mut hp_out = 0.0_f32;
	let mut hp_prev = 0.0_f32;
	let mut lp_out = 0.0_f32;
	let mut total_energy = 0.0_f32;
	let mut band_energy = 0.0_f32;

	for &sample in samples {
		total_energy += sample * sample;
		hp_out = hp_alpha * (hp_out + sample - hp_prev);
		hp_prev = sample;
		lp_out += lp_alpha * (hp_out - lp_out);
		band_energy += lp_out * lp_out;
	}

	if total_energy <= 0.0 {
		return 0.0;
	}

	(band_energy / total_energy).clamp(0.0, 1.0)
}

#[cfg(test)]
mod activity_tests {
	use super::{ActivityGate, ActivityMetrics, activity_metrics};
	use crate::settings::SttSettings;

	#[test]
	fn activity_metrics_detects_silence() {
		let samples = vec![0.0_f32; 160];
		let metrics = activity_metrics(&samples, 16_000);
		assert!(metrics.rms <= 1e-6);
		assert!(metrics.mean_abs <= 1e-6);
	}

	#[test]
	fn activity_metrics_zero_crossing_increases_with_alternation() {
		let mut samples = Vec::new();
		for i in 0..200 {
			let value = if i % 2 == 0 { 0.8 } else { -0.8 };
			samples.push(value);
		}
		let metrics = activity_metrics(&samples, 16_000);
		assert!(metrics.zero_crossing_rate > 0.4);
	}

	#[test]
	fn activity_gate_allows_high_energy_even_with_low_band_ratio() {
		let settings = SttSettings::default();
		let gate = ActivityGate::new(&settings.window);
		let metrics = ActivityMetrics {
			mean_abs: settings.window.min_mean_abs * 2.0,
			rms: settings.window.min_rms * 2.0,
			zero_crossing_rate: settings.window.max_zero_crossing_rate * 0.5,
			band_energy_ratio: settings.window.min_band_energy_ratio * 0.5,
		};
		assert!(gate.allows(&metrics));
	}

	#[test]
	fn activity_gate_allows_low_noise_with_low_band_ratio() {
		let settings = SttSettings::default();
		let gate = ActivityGate::new(&settings.window);
		let metrics = ActivityMetrics {
			mean_abs: settings.window.min_mean_abs * 1.1,
			rms: settings.window.min_rms * 1.1,
			zero_crossing_rate: settings.window.max_zero_crossing_rate * 0.2,
			band_energy_ratio: settings.window.min_band_energy_ratio * 0.1,
		};
		assert!(gate.allows(&metrics));
	}
}
