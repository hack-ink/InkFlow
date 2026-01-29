mod merge;
mod profiles;
mod sherpa;
mod whisper;
mod window;

pub use merge::MergeSettings;
pub use profiles::WhisperProfiles;
pub use sherpa::SherpaSettings;
pub use whisper::WhisperSettings;
pub use window::WhisperWindowSettings;

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

#[cfg(test)]
mod tests {
	use super::{SttSettings, WhisperSettings};

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
		let settings = SttSettings {
			second_pass_queue_capacity: 0,
			..Default::default()
		};
		assert!(settings.validate().is_err());
	}
}
