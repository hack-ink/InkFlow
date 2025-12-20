use std::{
	io::Write as _,
	process::{Command, Stdio},
};

use crate::{
	error::AppError,
	platform::{Platform, SystemSettingsTarget},
};

pub struct MacOsPlatform;

pub static MACOS_PLATFORM: MacOsPlatform = MacOsPlatform;

impl Platform for MacOsPlatform {
	fn overlay_shortcut(&self) -> &'static str {
		"alt+space"
	}

	fn apply_overlay_window_effects(&self, window: &tauri::WebviewWindow) -> Result<(), AppError> {
		window
			.with_webview(|webview| unsafe {
				use objc2::{msg_send, runtime::AnyObject, sel};
				use objc2_app_kit::{
					NSScrollView, NSStatusWindowLevel, NSWindow, NSWindowCollectionBehavior,
				};

				let ns_window: &NSWindow = &*webview.ns_window().cast();
				let mut collection_behavior = ns_window.collectionBehavior();
				collection_behavior.remove(NSWindowCollectionBehavior::CanJoinAllSpaces);
				collection_behavior.insert(NSWindowCollectionBehavior::MoveToActiveSpace);
				collection_behavior.insert(NSWindowCollectionBehavior::Transient);
				ns_window.setCollectionBehavior(collection_behavior);
				ns_window.setLevel(NSStatusWindowLevel);
				ns_window.setHidesOnDeactivate(true);

				let disable_scroll_bars = |scroll_view: &NSScrollView| {
					scroll_view.setHasVerticalScroller(false);
					scroll_view.setHasHorizontalScroller(false);
					scroll_view.setAutohidesScrollers(true);
					scroll_view.setDrawsBackground(false);
				};

				let wk_webview: &AnyObject = &*webview.inner().cast();
				let has_scroll_view: bool =
					msg_send![wk_webview, respondsToSelector: sel!(scrollView)];
				if has_scroll_view {
					let scroll_view: *mut NSScrollView = msg_send![wk_webview, scrollView];
					if let Some(scroll_view) = scroll_view.as_ref() {
						disable_scroll_bars(scroll_view);
					}
				}
			})
			.map_err(|err| {
				AppError::new(
					"overlay_window_effects_failed",
					format!("Failed to apply macOS overlay window effects: {err}."),
				)
			})?;

		Ok(())
	}

	fn inject_text_via_paste(&self, _text: &str) -> Result<(), AppError> {
		let mut pbcopy = Command::new("pbcopy")
			.stdin(Stdio::piped())
			.stdout(Stdio::null())
			.stderr(Stdio::piped())
			.spawn()
			.map_err(|err| {
				AppError::new("clipboard_set_failed", format!("Failed to start pbcopy: {err}."))
			})?;

		if let Some(mut stdin) = pbcopy.stdin.take() {
			stdin.write_all(_text.as_bytes()).map_err(|err| {
				AppError::new("clipboard_set_failed", format!("Failed to write to pbcopy: {err}."))
			})?;
		}

		let status = pbcopy.wait().map_err(|err| {
			AppError::new("clipboard_set_failed", format!("Failed to wait for pbcopy: {err}."))
		})?;

		if !status.success() {
			return Err(AppError::new(
				"clipboard_set_failed",
				"Failed to set the clipboard contents.",
			));
		}

		let output = Command::new("osascript")
			.args([
				"-e",
				"tell application \"System Events\" to keystroke \"v\" using command down",
			])
			.stdout(Stdio::null())
			.stderr(Stdio::piped())
			.output()
			.map_err(|err| {
				AppError::new("text_injection_failed", format!("Failed to run osascript: {err}."))
			})?;

		if output.status.success() {
			return Ok(());
		}

		let stderr = String::from_utf8_lossy(&output.stderr);
		if stderr.to_lowercase().contains("not authorized")
			|| stderr.to_lowercase().contains("not authorised")
			|| stderr.to_lowercase().contains("not permitted")
		{
			return Err(AppError::new(
				"accessibility_permission_required",
				"Text injection requires Accessibility permission. Enable it in System Settings > Privacy & Security > Accessibility, then try again.",
			));
		}

		Err(AppError::new("text_injection_failed", format!("Text injection failed: {stderr}.")))
	}

	fn inject_text_via_typing(&self, _text: &str) -> Result<(), AppError> {
		Err(AppError::new("not_implemented", "Text injection is not implemented yet."))
	}

	fn open_system_settings(&self, target: SystemSettingsTarget) -> Result<(), AppError> {
		let url = match target {
			SystemSettingsTarget::Microphone =>
				"x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
			SystemSettingsTarget::Accessibility =>
				"x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
			SystemSettingsTarget::InputMonitoring =>
				"x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent",
		};

		Command::new("open").arg(url).spawn().map_err(|err| {
			AppError::new(
				"open_system_settings_failed",
				format!("Failed to open system settings: {err}."),
			)
		})?;

		Ok(())
	}
}
