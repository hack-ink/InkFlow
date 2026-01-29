use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WhisperProfiles {
	pub window_best_of: u8,
	pub second_pass_best_of: u8,
}

impl Default for WhisperProfiles {
	fn default() -> Self {
		Self { window_best_of: 1, second_pass_best_of: 5 }
	}
}

impl WhisperProfiles {
	pub(crate) fn validate(&self) -> Result<(), AppError> {
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
