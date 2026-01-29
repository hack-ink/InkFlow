use std::{
	ffi::CString,
	os::raw::{c_char, c_void},
	sync::{
		Arc, Mutex,
		atomic::{AtomicBool, Ordering},
	},
	thread,
	time::Duration,
};

use inkflow_core::InkFlowEngine;

use crate::json::{error_json, update_to_json};
use crate::types::{CallbackState, UserData};

pub(crate) fn stop_callback(lock: &Mutex<Option<CallbackState>>) {
	if let Ok(mut guard) = lock.lock() {
		stop_callback_inner(&mut guard);
	}
}

pub(crate) fn stop_callback_inner(guard: &mut Option<CallbackState>) {
	if let Some(state) = guard.take() {
		state.stop.store(true, Ordering::SeqCst);
		let _ = state.thread.join();
	}
}

pub(crate) fn callback_loop(
	engine: Arc<Mutex<Option<InkFlowEngine>>>,
	callback: extern "C" fn(*const c_char, *mut c_void),
	user_data: UserData,
	stop: Arc<AtomicBool>,
) {
	const IDLE_SLEEP_MS: u64 = 12;

	loop {
		if stop.load(Ordering::SeqCst) {
			return;
		}

		let next = {
			let guard = match engine.lock() {
				Ok(guard) => guard,
				Err(_) => {
					send_payload(
						callback,
						user_data,
						&error_json("engine_lock_failed", "Engine lock poisoned."),
					);
					return;
				},
			};

			let Some(engine) = guard.as_ref() else {
				send_payload(
					callback,
					user_data,
					&error_json("engine_missing", "Engine is not available."),
				);
				return;
			};

			engine.poll_update()
		};

		match next {
			Ok(Some(update)) => {
				let payload = update_to_json(update);
				send_payload(callback, user_data, &payload);
			},
			Ok(None) => {
				thread::sleep(Duration::from_millis(IDLE_SLEEP_MS));
			},
			Err(err) => {
				let payload = error_json(&err.code, &err.message);
				send_payload(callback, user_data, &payload);
				thread::sleep(Duration::from_millis(IDLE_SLEEP_MS));
			},
		}
	}
}

fn send_payload(
	callback: extern "C" fn(*const c_char, *mut c_void),
	user_data: UserData,
	payload: &str,
) {
	let Ok(c_string) = CString::new(payload) else {
		return;
	};
	callback(c_string.as_ptr(), user_data.0);
}
