use serde::{Deserialize, Serialize};

use crate::error::AppError;

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
	pub(crate) fn validate(&self) -> Result<(), AppError> {
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
