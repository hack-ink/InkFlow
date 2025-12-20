#[cfg(target_os = "linux")] mod linux;
#[cfg(target_os = "macos")] mod macos;
#[cfg(windows)] mod windows;

use serde::Deserialize;

use crate::error::AppError;

pub trait Platform
where
	Self: Send + Sync,
{
	fn overlay_shortcut(&self) -> &'static str;

	fn apply_overlay_window_effects(&self, window: &tauri::WebviewWindow) -> Result<(), AppError>;

	fn inject_text_via_paste(&self, text: &str) -> Result<(), AppError>;

	#[allow(dead_code)]
	fn inject_text_via_typing(&self, text: &str) -> Result<(), AppError>;

	fn open_system_settings(&self, target: SystemSettingsTarget) -> Result<(), AppError>;
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemSettingsTarget {
	Microphone,
	Accessibility,
	InputMonitoring,
}

#[allow(dead_code)]
struct UnsupportedPlatform;

#[allow(dead_code)]
static UNSUPPORTED_PLATFORM: UnsupportedPlatform = UnsupportedPlatform;

impl Platform for UnsupportedPlatform {
	fn overlay_shortcut(&self) -> &'static str {
		"alt+space"
	}

	fn apply_overlay_window_effects(&self, _window: &tauri::WebviewWindow) -> Result<(), AppError> {
		Ok(())
	}

	fn inject_text_via_paste(&self, _text: &str) -> Result<(), AppError> {
		Err(AppError::new(
			"unsupported_platform",
			"Text injection is not supported on this platform build.",
		))
	}

	fn inject_text_via_typing(&self, _text: &str) -> Result<(), AppError> {
		Err(AppError::new(
			"unsupported_platform",
			"Text injection is not supported on this platform build.",
		))
	}

	fn open_system_settings(&self, _target: SystemSettingsTarget) -> Result<(), AppError> {
		Err(AppError::new(
			"unsupported_platform",
			"Opening system settings is not supported on this platform build.",
		))
	}
}

pub fn current() -> &'static dyn Platform {
	#[cfg(target_os = "macos")]
	{
		&macos::MACOS_PLATFORM
	}

	#[cfg(windows)]
	{
		&windows::WINDOWS_PLATFORM
	}

	#[cfg(target_os = "linux")]
	{
		&linux::LINUX_PLATFORM
	}

	#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
	{
		&UNSUPPORTED_PLATFORM
	}
}
