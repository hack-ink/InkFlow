use tauri::Manager as _;

use crate::{
	events::{
		ErrorEvent, LlmRewriteEvent, SessionStateEvent, SttFinalEvent, SttPartialEvent, emit_error,
		emit_llm_rewrite, emit_session_state, emit_stt_final, emit_stt_partial,
	},
	ports::UiPort,
};

pub(crate) struct UiAdapter {
	app: tauri::AppHandle,
}
impl UiAdapter {
	pub(crate) fn new(app: tauri::AppHandle) -> Self {
		Self { app }
	}
}
impl UiPort for UiAdapter {
	fn emit_session_state(&self, payload: SessionStateEvent) {
		emit_session_state(&self.app, payload);
	}

	fn emit_stt_partial(&self, payload: SttPartialEvent) {
		emit_stt_partial(&self.app, payload);
	}

	fn emit_stt_final(&self, payload: SttFinalEvent) {
		emit_stt_final(&self.app, payload);
	}

	fn emit_llm_rewrite(&self, payload: LlmRewriteEvent) {
		emit_llm_rewrite(&self.app, payload);
	}

	fn emit_error(&self, payload: ErrorEvent) {
		emit_error(&self.app, payload);
	}

	fn hide_overlay_window(&self) {
		let Some(window) = self.app.get_webview_window("main") else {
			return;
		};

		if let (Ok(size), Ok(scale_factor)) = (window.inner_size(), window.scale_factor()) {
			let height = (crate::overlay::COLLAPSED_HEIGHT_LOGICAL * scale_factor).round() as u32;
			if let Err(err) = window.set_size(tauri::PhysicalSize::new(size.width, height)) {
				eprintln!("Failed to reset the overlay window size: {err}.");
			}
		}

		if let Err(err) = window.hide() {
			eprintln!("Failed to hide the overlay window: {err}.");
		}
	}

	fn show_overlay_window(&self) {
		let Some(window) = self.app.get_webview_window("main") else {
			return;
		};

		if let Err(err) = crate::platform::current().apply_overlay_window_effects(&window) {
			eprintln!("Failed to apply overlay window effects: {}.", err.message);
		}

		let _ = window.show().and_then(|_| window.set_focus());
	}
}
