use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SttSettings {
	pub sherpa: SherpaSettings,
	pub whisper: WhisperSettings,
	pub window: WhisperWindowSettings,
	pub merge: MergeSettings,
	pub profiles: WhisperProfiles,
	pub second_pass_queue_capacity: usize,
}

impl Default for SttSettings {
	fn default() -> Self {
		Self {
			sherpa: SherpaSettings::default(),
			whisper: WhisperSettings::default(),
			window: WhisperWindowSettings::default(),
			merge: MergeSettings::default(),
			profiles: WhisperProfiles::default(),
			second_pass_queue_capacity: 16,
		}
	}
}

impl SttSettings {
	pub fn validate(&self) -> Result<(), AppError> {
		self.sherpa.validate()?;
		self.whisper.validate()?;
		self.window.validate()?;
		self.merge.validate()?;
		self.profiles.validate()?;
		self.validate_queue_limits()?;

		if self.window.enabled && self.window.window_ms < self.window.step_ms {
			return Err(AppError::new(
				"settings_invalid",
				"window_ms must be greater than or equal to step_ms.",
			));
		}

		Ok(())
	}

	fn validate_queue_limits(&self) -> Result<(), AppError> {
		if self.window.window_backpressure_high_watermark == 0 {
			return Err(AppError::new(
				"settings_invalid",
				"window_backpressure_high_watermark must be greater than zero.",
			));
		}

		if self.second_pass_queue_capacity == 0 {
			return Err(AppError::new(
				"settings_invalid",
				"second_pass_queue_capacity must be greater than zero.",
			));
		}

		Ok(())
	}
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SherpaSettings {
	pub model_dir: String,
	pub provider: String,
	pub num_threads: Option<i32>,
	pub decoding_method: String,
	pub max_active_paths: i32,
	pub rule1_min_trailing_silence: f32,
	pub rule2_min_trailing_silence: f32,
	pub rule3_min_utterance_length: f32,
	pub prefer_int8: bool,
	pub use_int8_decoder: bool,
	pub chunk_ms: u32,
}

impl Default for SherpaSettings {
	fn default() -> Self {
		Self {
			model_dir: String::new(),
			provider: "cpu".into(),
			num_threads: None,
			decoding_method: "greedy_search".into(),
			max_active_paths: 4,
			rule1_min_trailing_silence: 2.4,
			rule2_min_trailing_silence: 1.2,
			rule3_min_utterance_length: 300.0,
			prefer_int8: true,
			use_int8_decoder: false,
			chunk_ms: 170,
		}
	}
}

impl SherpaSettings {
	fn validate(&self) -> Result<(), AppError> {
		if self.provider.trim().is_empty() {
			return Err(AppError::new("settings_invalid", "Sherpa provider must not be empty."));
		}

		match self.decoding_method.as_str() {
			"greedy_search" | "modified_beam_search" => {},
			other => {
				return Err(AppError::new(
					"settings_invalid",
					format!(
						"Invalid sherpa decoding method: {other:?}. Expected \"greedy_search\" or \"modified_beam_search\"."
					),
				));
			},
		}

		if let Some(threads) = self.num_threads
			&& threads <= 0
		{
			return Err(AppError::new(
				"settings_invalid",
				"Sherpa num_threads must be a positive integer when set.",
			));
		}

		if self.max_active_paths <= 0 {
			return Err(AppError::new(
				"settings_invalid",
				"Sherpa max_active_paths must be a positive integer.",
			));
		}

		if !self.rule1_min_trailing_silence.is_finite() || self.rule1_min_trailing_silence <= 0.0 {
			return Err(AppError::new(
				"settings_invalid",
				"Sherpa rule1_min_trailing_silence must be a positive, finite number.",
			));
		}

		if !self.rule2_min_trailing_silence.is_finite() || self.rule2_min_trailing_silence <= 0.0 {
			return Err(AppError::new(
				"settings_invalid",
				"Sherpa rule2_min_trailing_silence must be a positive, finite number.",
			));
		}

		if !self.rule3_min_utterance_length.is_finite() || self.rule3_min_utterance_length <= 0.0 {
			return Err(AppError::new(
				"settings_invalid",
				"Sherpa rule3_min_utterance_length must be a positive, finite number.",
			));
		}

		if self.chunk_ms < 40 || self.chunk_ms > 400 {
			return Err(AppError::new(
				"settings_invalid",
				"Sherpa chunk_ms must be between 40 and 400.",
			));
		}

		Ok(())
	}
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WhisperSettings {
	pub model_path: String,
	pub language: String,
	pub num_threads: Option<i32>,
	pub force_gpu: Option<bool>,
}

impl Default for WhisperSettings {
	fn default() -> Self {
		Self {
			model_path: String::new(),
			language: "auto".into(),
			num_threads: None,
			force_gpu: None,
		}
	}
}

impl WhisperSettings {
	fn validate(&self) -> Result<(), AppError> {
		if self.language.contains('\0') {
			return Err(AppError::new(
				"settings_invalid",
				"Whisper language must not contain NUL bytes.",
			));
		}

		if let Some(threads) = self.num_threads
			&& threads <= 0
		{
			return Err(AppError::new(
				"settings_invalid",
				"Whisper num_threads must be a positive integer when set.",
			));
		}

		Ok(())
	}
}

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
	fn validate(&self) -> Result<(), AppError> {
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MergeSettings {
	pub stable_ticks: u32,
	pub rollback_threshold_tokens: u32,
	pub overlap_k_words: u32,
	pub overlap_k_chars: u32,
}

impl Default for MergeSettings {
	fn default() -> Self {
		Self {
			stable_ticks: 3,
			rollback_threshold_tokens: 8,
			overlap_k_words: 30,
			overlap_k_chars: 100,
		}
	}
}

impl MergeSettings {
	fn validate(&self) -> Result<(), AppError> {
		if self.stable_ticks == 0 {
			return Err(AppError::new(
				"settings_invalid",
				"stable_ticks must be greater than zero.",
			));
		}

		if self.overlap_k_words == 0 || self.overlap_k_chars == 0 {
			return Err(AppError::new(
				"settings_invalid",
				"Overlap limits must be greater than zero.",
			));
		}

		Ok(())
	}
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WhisperProfiles {
	pub window_best_of: u8,
	pub second_pass_best_of: u8,
}

#[cfg(test)]
mod tests {
	use super::WhisperSettings;
	use super::SttSettings;

	#[test]
	fn whisper_default_language_is_auto() {
		let settings = WhisperSettings::default();
		assert_eq!(settings.language, "auto");
	}

	#[test]
	fn window_backpressure_high_watermark_must_be_positive() {
		let mut settings = SttSettings::default();
		settings.window.window_backpressure_high_watermark = 0;
		assert!(settings.validate().is_err());
	}

	#[test]
	fn second_pass_queue_capacity_must_be_positive() {
		let mut settings = SttSettings::default();
		settings.second_pass_queue_capacity = 0;
		assert!(settings.validate().is_err());
	}
}

impl Default for WhisperProfiles {
	fn default() -> Self {
		Self { window_best_of: 1, second_pass_best_of: 5 }
	}
}

impl WhisperProfiles {
	fn validate(&self) -> Result<(), AppError> {
		for (name, value) in [
			("window_best_of", self.window_best_of),
			("second_pass_best_of", self.second_pass_best_of),
		] {
			if value == 0 || value > 8 {
				return Err(AppError::new(
					"settings_invalid",
					format!("{name} must be within 1..=8."),
				));
			}
		}

		Ok(())
	}
}
