mod adapters;
mod app_state;
mod application;
mod audio;
mod commands;
mod domain;
mod engine;
mod error;
mod events;
mod llm;
mod overlay;
mod pipeline;
mod platform;
mod ports;
mod session;
mod settings;
mod stt;
mod stt_trace;

use tauri::Manager;

use crate::{app_state::AppState, session::SessionAction};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
	if let Err(err) = tauri::Builder::default()
		.manage(AppState::new())
		.invoke_handler(tauri::generate_handler![
			commands::overlay_set_height,
			commands::session_dispatch,
			commands::settings_get,
			commands::settings_update,
			commands::engine_apply_settings,
			commands::platform_open_system_settings
		])
		.on_window_event(|window, event| {
			if window.label() != "main" {
				return;
			}

			if let tauri::WindowEvent::Focused(false) = event {
				let app_handle = window.app_handle().clone();
				tauri::async_runtime::spawn(async move {
					let state = app_handle.state::<AppState>();
					let _ = state.session().dispatch(&app_handle, SessionAction::Escape).await;
				});
			}
		})
		.setup(|app| {
			let whisper_logs_enabled = std::env::var("AIR_WHISPER_LOGS")
				.ok()
				.and_then(|value| match value.trim().to_lowercase().as_str() {
					"1" | "true" | "yes" | "y" | "on" => Some(true),
					"0" | "false" | "no" | "n" | "off" => Some(false),
					_ => None,
				})
				.unwrap_or(false);

			whisper_rs::install_logging_hooks();

			if cfg!(debug_assertions) {
				let mut log_builder =
					tauri_plugin_log::Builder::default().level(log::LevelFilter::Info);
				if !whisper_logs_enabled {
					log_builder = log_builder.level_for("whisper_rs", log::LevelFilter::Off);
				}
				app.handle().plugin(log_builder.build())?;
			}

			if let Some(window) = app.get_webview_window("main")
				&& let Err(err) = crate::platform::current().apply_overlay_window_effects(&window)
			{
				eprintln!("Failed to apply overlay window effects: {}.", err.message);
			}

			#[cfg(desktop)]
			{
				use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};

				let overlay_shortcut = crate::platform::current().overlay_shortcut();
				app.handle().plugin(
					tauri_plugin_global_shortcut::Builder::new()
						.with_shortcuts([overlay_shortcut])?
						.with_handler(move |app, shortcut, event| {
							if event.state != ShortcutState::Pressed {
								return;
							}

							if !shortcut.matches(Modifiers::ALT, Code::Space) {
								return;
							}

							let Some(window) = app.get_webview_window("main") else {
								eprintln!("Overlay window is not available.");
								return;
							};

							let is_visible = match window.is_visible() {
								Ok(is_visible) => is_visible,
								Err(err) => {
									eprintln!(
										"Failed to read the overlay window visibility: {err}."
									);
									false
								},
							};

							if is_visible {
								let app_handle = app.clone();
								tauri::async_runtime::spawn(async move {
									let state = app_handle.state::<AppState>();
									let _ = state
										.session()
										.dispatch(&app_handle, SessionAction::Escape)
										.await;
								});
								return;
							}

							if let (Ok(size), Ok(scale_factor)) =
								(window.inner_size(), window.scale_factor())
							{
								let height = (crate::overlay::COLLAPSED_HEIGHT_LOGICAL
									* scale_factor)
									.round() as u32;
								if let Err(err) =
									window.set_size(tauri::PhysicalSize::new(size.width, height))
								{
									eprintln!("Failed to reset the overlay window size: {err}.");
								}
							}

							if let Err(err) =
								crate::platform::current().apply_overlay_window_effects(&window)
							{
								eprintln!(
									"Failed to apply overlay window effects: {}",
									err.message
								);
							}

							if let Err(err) = window.show().and_then(|_| window.set_focus()) {
								eprintln!("Failed to show the overlay window: {err}.");
								return;
							}

							let app_handle = app.clone();
							tauri::async_runtime::spawn(async move {
								let state = app_handle.state::<AppState>();
								let _ = state
									.session()
									.dispatch(&app_handle, SessionAction::Show)
									.await;
							});
						})
						.build(),
				)?;
			}

			Ok(())
		})
		.run(tauri::generate_context!())
	{
		eprintln!("Failed to run the Tauri application: {err}.");
		std::process::exit(1);
	}
}
