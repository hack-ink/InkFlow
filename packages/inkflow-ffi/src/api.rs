use std::{
	os::raw::c_void,
	ptr,
	sync::{
		Arc, Mutex,
		atomic::AtomicBool,
	},
	thread,
};

use inkflow_core::{AppError, InkFlowEngine, SttSettings};

use crate::callbacks::{callback_loop, stop_callback, stop_callback_inner};
use crate::logging;
use crate::types::{
	CallbackState, InkFlowHandle, InkFlowStatus, InkFlowUpdateCallback, UserData,
};

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
/// The caller must pass a valid pointer returned by `inkflow_engine_create`.
pub unsafe extern "C" fn inkflow_engine_force_finalize(handle: *mut InkFlowHandle) -> i32 {
	if handle.is_null() {
		return InkFlowStatus::Null.code();
	}

	let engine = unsafe { &*handle }.engine.clone();
	let guard = match engine.lock() {
		Ok(guard) => guard,
		Err(_) => return InkFlowStatus::InternalError.code(),
	};
	let Some(engine) = guard.as_ref() else {
		return InkFlowStatus::InternalError.code();
	};

	match engine.force_finalize() {
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

fn map_error_status(err: &AppError) -> InkFlowStatus {
	match err.code.as_str() {
		"audio_invalid" | "settings_invalid" => InkFlowStatus::InvalidArgument,
		_ => InkFlowStatus::InternalError,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn force_finalize_returns_null_when_handle_missing() {
		let status = unsafe { inkflow_engine_force_finalize(ptr::null_mut()) };
		assert_eq!(status, InkFlowStatus::Null.code());
	}
}
