use tauri::{Manager as _, State};

#[cfg(not(target_os = "macos"))] use tauri::PhysicalSize;

use crate::{
	app_state::AppState,
	engine::ApplyLevel,
	error::AppError,
	platform::SystemSettingsTarget,
	session::{SessionAction, SessionSnapshot},
	settings::{SettingsPatch, SettingsPublic},
};

#[derive(Clone, Debug, serde::Serialize)]
pub struct EngineApplyResponse {
	pub apply_level: ApplyLevel,
	pub settings: SettingsPublic,
}

#[tauri::command]
pub async fn overlay_set_height(
	app: tauri::AppHandle,
	height: f64,
	animate: bool,
) -> Result<(), AppError> {
	let Some(window) = app.get_webview_window("main") else {
		return Ok(());
	};

	if let Err(err) = crate::platform::current().apply_overlay_window_effects(&window) {
		eprintln!("Failed to apply overlay window effects: {}", err.message);
	}

	#[cfg(target_os = "macos")]
	{
		window
			.with_webview(move |webview| unsafe {
				use objc2_app_kit::NSWindow;

				let ns_window: &NSWindow = &*webview.ns_window().cast();
				let mut frame = ns_window.frame();
				let start_height = frame.size.height;
				let target_height = height.max(crate::overlay::COLLAPSED_HEIGHT_LOGICAL);
				if (start_height - target_height).abs() < 0.5 {
					return;
				}

				frame.origin.y += start_height - target_height;
				frame.size.height = target_height;
				ns_window.setFrame_display_animate(frame, true, animate);
			})
			.map_err(|err| {
				AppError::new(
					"overlay_resize_failed",
					format!("Failed to resize the overlay window: {err}."),
				)
			})?;

		Ok(())
	}

	#[cfg(not(target_os = "macos"))]
	{
		let _ = animate;
		let scale_factor = window.scale_factor().map_err(|err| {
			AppError::new(
				"overlay_resize_failed",
				format!("Failed to read the overlay window scale factor: {err}."),
			)
		})?;

		let target_height_logical = height.max(crate::overlay::COLLAPSED_HEIGHT_LOGICAL);
		let target_height = (target_height_logical * scale_factor).round() as u32;
		let start_size = window.inner_size().map_err(|err| {
			AppError::new(
				"overlay_resize_failed",
				format!("Failed to read the overlay window size: {err}."),
			)
		})?;
		if start_size.height != target_height {
			window.set_size(PhysicalSize::new(start_size.width, target_height)).map_err(|err| {
				AppError::new(
					"overlay_resize_failed",
					format!("Failed to resize the overlay window: {err}."),
				)
			})?;
		}

		Ok(())
	}
}

#[tauri::command]
pub async fn session_dispatch(
	app: tauri::AppHandle,
	state: State<'_, AppState>,
	action: SessionAction,
) -> Result<SessionSnapshot, AppError> {
	state.session().dispatch(&app, action).await
}

#[tauri::command]
pub async fn settings_get(
	app: tauri::AppHandle,
	state: State<'_, AppState>,
) -> Result<SettingsPublic, AppError> {
	state.settings().get_public(&app).await
}

#[tauri::command]
pub async fn settings_update(
	app: tauri::AppHandle,
	state: State<'_, AppState>,
	patch: SettingsPatch,
) -> Result<SettingsPublic, AppError> {
	state.settings().update(&app, patch).await
}

#[tauri::command]
pub async fn engine_apply_settings(
	app: tauri::AppHandle,
	state: State<'_, AppState>,
	patch: SettingsPatch,
) -> Result<EngineApplyResponse, AppError> {
	let apply = state.settings().apply_patch(&app, patch).await?;

	let required_level = state.engine().classify_apply_level(&apply.old_stt, &apply.new_stt).await;
	let is_busy = state.session().is_listening_or_finalizing().await;
	if required_level == ApplyLevel::Reloaded && is_busy {
		return Err(AppError::new(
			"engine_reload_disallowed_while_listening",
			"Stop dictation before applying these speech engine settings.",
		));
	}

	let apply_level = state.engine().apply_settings(&app, &apply.old_stt, &apply.new_stt).await?;
	Ok(EngineApplyResponse { apply_level, settings: apply.public })
}

#[tauri::command]
pub async fn platform_open_system_settings(target: SystemSettingsTarget) -> Result<(), AppError> {
	crate::platform::current().open_system_settings(target)
}
