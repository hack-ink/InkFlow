use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WhisperWindowSettings {
	pub enabled: bool,
	pub window_ms: u64,
	pub step_ms: u64,
	pub context_ms: u64,
	pub min_mean_abs: f32,
	pub min_rms: f32,
	pub max_zero_crossing_rate: f32,
	pub min_band_energy_ratio: f32,
	pub emit_every: u32,
	pub endpoint_tail_ms: u64,
	pub window_backpressure_high_watermark: usize,
}

impl Default for WhisperWindowSettings {
	fn default() -> Self {
		Self {
			enabled: true,
			window_ms: 4000,
			step_ms: 400,
			context_ms: 800,
			min_mean_abs: 0.001,
			min_rms: 0.001,
			max_zero_crossing_rate: 0.35,
			min_band_energy_ratio: 0.15,
			emit_every: 1,
			endpoint_tail_ms: 200,
			window_backpressure_high_watermark: 16,
		}
	}
}

impl WhisperWindowSettings {
	pub(crate) fn validate(&self) -> Result<(), AppError> {
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

		if !self.min_rms.is_finite() || self.min_rms < 0.0 {
			return Err(AppError::new(
				"settings_invalid",
				"min_rms must be a finite number greater than or equal to zero.",
			));
		}

		if !self.max_zero_crossing_rate.is_finite()
			|| self.max_zero_crossing_rate < 0.0
			|| self.max_zero_crossing_rate > 1.0
		{
			return Err(AppError::new(
				"settings_invalid",
				"max_zero_crossing_rate must be a finite number between 0.0 and 1.0.",
			));
		}

		if !self.min_band_energy_ratio.is_finite()
			|| self.min_band_energy_ratio < 0.0
			|| self.min_band_energy_ratio > 1.0
		{
			return Err(AppError::new(
				"settings_invalid",
				"min_band_energy_ratio must be a finite number between 0.0 and 1.0.",
			));
		}

		if self.emit_every == 0 {
			return Err(AppError::new("settings_invalid", "emit_every must be greater than zero."));
		}

		Ok(())
	}
}
