use serde::{Deserialize, Serialize};

use crate::error::AppError;

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
	pub(crate) fn validate(&self) -> Result<(), AppError> {
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
