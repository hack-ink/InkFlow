use serde::{Deserialize, Serialize};

use crate::error::AppError;

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
	pub(crate) fn validate(&self) -> Result<(), AppError> {
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
