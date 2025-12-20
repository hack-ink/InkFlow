use crate::{
	error::AppError,
	platform::{Platform, SystemSettingsTarget},
};

pub struct LinuxPlatform;

pub static LINUX_PLATFORM: LinuxPlatform = LinuxPlatform;

impl Platform for LinuxPlatform {
	fn overlay_shortcut(&self) -> &'static str {
		"alt+space"
	}

	fn apply_overlay_window_effects(&self, _window: &tauri::WebviewWindow) -> Result<(), AppError> {
		Err(AppError::new(
			"not_supported",
			"Window effects are not supported by this placeholder implementation.",
		))
	}

	fn inject_text_via_paste(&self, _text: &str) -> Result<(), AppError> {
		Err(AppError::new(
			"not_supported",
			"Text injection is not supported by this placeholder implementation.",
		))
	}

	fn inject_text_via_typing(&self, _text: &str) -> Result<(), AppError> {
		Err(AppError::new(
			"not_supported",
			"Text injection is not supported by this placeholder implementation.",
		))
	}

	fn open_system_settings(&self, _target: SystemSettingsTarget) -> Result<(), AppError> {
		Err(AppError::new(
			"not_supported",
			"Opening system settings is not supported by this placeholder implementation.",
		))
	}
}
