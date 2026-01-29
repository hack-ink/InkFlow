mod logging;

use std::{
	ffi::CString,
	os::raw::{c_char, c_void},
	ptr,
	sync::{
		Arc, Mutex,
		atomic::{AtomicBool, Ordering},
	},
	thread,
	time::Duration,
};

use inkflow_core::{AppError, AsrUpdate, InkFlowEngine, SttSettings};
use serde_json::json;

#[repr(i32)]
enum InkFlowStatus {
	Ok = 0,
	Null = 1,
	InvalidArgument = 2,
	InternalError = 3,
}

impl InkFlowStatus {
	fn code(self) -> i32 {
		self as i32
	}
}

type InkFlowUpdateCallback = Option<extern "C" fn(*const c_char, *mut c_void)>;

#[derive(Clone, Copy)]
struct UserData(*mut c_void);

unsafe impl Send for UserData {}

struct CallbackState {
	stop: Arc<AtomicBool>,
	thread: thread::JoinHandle<()>,
}

#[repr(C)]
pub struct InkFlowHandle {
	engine: Arc<Mutex<Option<InkFlowEngine>>>,
	callback: Mutex<Option<CallbackState>>,
}

#[unsafe(no_mangle)]
pub extern "C" fn inkflow_engine_create() -> *mut InkFlowHandle {
	logging::init();
	match InkFlowEngine::start(SttSettings::default()) {
		Ok(engine) => Box::into_raw(Box::new(InkFlowHandle {
			engine: Arc::new(Mutex::new(Some(engine))),
			callback: Mutex::new(None),
		})),
		Err(err) => {
			tracing::error!(error = %err.message, "InkFlow engine initialization failed.");
			ptr::null_mut()
		},
	}
}

#[unsafe(no_mangle)]
/// # Safety
/// The caller must pass a valid pointer returned by `inkflow_engine_create`.
/// If the pointer is non-null, it must not be used after this call returns.
pub unsafe extern "C" fn inkflow_engine_destroy(handle: *mut InkFlowHandle) {
	if handle.is_null() {
		return;
	}

	let boxed = unsafe { Box::from_raw(handle) };
	stop_callback(&boxed.callback);

	let engine = boxed.engine.clone();
	let mut guard = match engine.lock() {
		Ok(guard) => guard,
		Err(_) => {
			tracing::error!("InkFlow engine lock poisoned during shutdown.");
			return;
		},
	};

	if let Some(engine) = guard.take()
		&& let Err(err) = engine.stop() {
			tracing::error!(error = %err.message, "InkFlow engine shutdown failed.");
		}
}

#[unsafe(no_mangle)]
/// # Safety
/// The caller must pass a valid `handle` pointer returned by `inkflow_engine_create`.
/// If `samples` is non-null, it must point to at least `sample_count` valid `f32` values.
pub unsafe extern "C" fn inkflow_engine_submit_audio(
	handle: *mut InkFlowHandle,
	samples: *const f32,
	sample_count: usize,
	sample_rate_hz: u32,
) -> i32 {
	if handle.is_null() {
		return InkFlowStatus::Null.code();
	}
	if sample_count == 0 {
		return InkFlowStatus::Ok.code();
	}
	if samples.is_null() {
		return InkFlowStatus::InvalidArgument.code();
	}

	let slice = unsafe { std::slice::from_raw_parts(samples, sample_count) };
	let engine = unsafe { &*handle }.engine.clone();
	let guard = match engine.lock() {
		Ok(guard) => guard,
		Err(_) => return InkFlowStatus::InternalError.code(),
	};
	let Some(engine) = guard.as_ref() else {
		return InkFlowStatus::InternalError.code();
	};

	match engine.submit_audio(slice, sample_rate_hz) {
		Ok(()) => InkFlowStatus::Ok.code(),
		Err(err) => map_error_status(&err).code(),
	}
}

#[unsafe(no_mangle)]
/// # Safety
/// The caller must pass a valid `handle` pointer returned by `inkflow_engine_create`.
/// The `callback` pointer must be a valid function pointer for the duration of the callback thread.
pub unsafe extern "C" fn inkflow_engine_register_callback(
	handle: *mut InkFlowHandle,
	callback: InkFlowUpdateCallback,
	user_data: *mut c_void,
) -> i32 {
	if handle.is_null() {
		return InkFlowStatus::Null.code();
	}
	let Some(callback) = callback else {
		return InkFlowStatus::InvalidArgument.code();
	};

	let handle = unsafe { &*handle };
	let mut guard = match handle.callback.lock() {
		Ok(guard) => guard,
		Err(_) => return InkFlowStatus::InternalError.code(),
	};

	stop_callback_inner(&mut guard);

	let engine = handle.engine.clone();
	let stop = Arc::new(AtomicBool::new(false));
	let stop_flag = stop.clone();

	let user_data = UserData(user_data);
	let thread = match thread::Builder::new()
		.name("inkflow-callback".into())
		.spawn(move || callback_loop(engine, callback, user_data, stop_flag))
	{
		Ok(thread) => thread,
		Err(_) => return InkFlowStatus::InternalError.code(),
	};

	*guard = Some(CallbackState { stop, thread });
	InkFlowStatus::Ok.code()
}

#[unsafe(no_mangle)]
/// # Safety
/// The caller must pass a valid pointer returned by `inkflow_engine_create`.
pub unsafe extern "C" fn inkflow_engine_unregister_callback(handle: *mut InkFlowHandle) {
	if handle.is_null() {
		return;
	}

	let handle = unsafe { &*handle };
	stop_callback(&handle.callback);
}

fn stop_callback(lock: &Mutex<Option<CallbackState>>) {
	if let Ok(mut guard) = lock.lock() {
		stop_callback_inner(&mut guard);
	}
}

fn stop_callback_inner(guard: &mut Option<CallbackState>) {
	if let Some(state) = guard.take() {
		state.stop.store(true, Ordering::SeqCst);
		let _ = state.thread.join();
	}
}

fn callback_loop(
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

fn map_error_status(err: &AppError) -> InkFlowStatus {
	match err.code.as_str() {
		"audio_invalid" | "settings_invalid" => InkFlowStatus::InvalidArgument,
		_ => InkFlowStatus::InternalError,
	}
}

fn update_to_json(update: AsrUpdate) -> String {
	match update {
		AsrUpdate::LiveRender { text } => json!({
			"kind": "live_render",
			"text": text,
		})
		.to_string(),
		AsrUpdate::SherpaPartial(text) =>
			json!({"kind": "sherpa_partial", "text": text}).to_string(),
		AsrUpdate::WindowScheduled(snapshot) => json!({
			"kind": "window_scheduled",
			"snapshot": snapshot_json(snapshot),
		})
		.to_string(),
		AsrUpdate::WindowResult { snapshot, result } => json!({
			"kind": "window_result",
			"snapshot": snapshot_json(snapshot),
			"result": {
				"text": result.text,
				"has_timestamps": result.has_timestamps,
				"segments": result.segments.iter().map(|segment| {
					json!({
						"t0_ms": segment.t0_ms,
						"t1_ms": segment.t1_ms,
						"text": segment.text,
					})
				}).collect::<Vec<_>>(),
			}
		})
		.to_string(),
		AsrUpdate::SegmentEnd {
			segment_id,
			sherpa_text,
			committed_end_16k_samples,
			window_generation_after,
		} => json!({
			"kind": "segment_end",
			"segment_id": segment_id,
			"text": sherpa_text,
			"committed_end_16k_samples": committed_end_16k_samples,
			"window_generation_after": window_generation_after,
		})
		.to_string(),
		AsrUpdate::EndpointReset { window_generation_after } => json!({
			"kind": "endpoint_reset",
			"window_generation_after": window_generation_after,
		})
		.to_string(),
		AsrUpdate::SecondPass { segment_id, text } => json!({
			"kind": "second_pass",
			"segment_id": segment_id,
			"text": text,
		})
		.to_string(),
	}
}

fn snapshot_json(snapshot: inkflow_core::stt::WindowJobSnapshot) -> serde_json::Value {
	json!({
		"engine_generation": snapshot.engine_generation,
		"window_generation": snapshot.window_generation,
		"job_id": snapshot.job_id,
		"window_end_16k_samples": snapshot.window_end_16k_samples,
		"window_len_16k_samples": snapshot.window_len_16k_samples,
		"context_len_16k_samples": snapshot.context_len_16k_samples,
	})
}

fn error_json(code: &str, message: &str) -> String {
	json!({"kind": "error", "code": code, "message": message}).to_string()
}
