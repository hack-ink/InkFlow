use std::{
	os::raw::{c_char, c_void},
	sync::{Arc, Mutex, atomic::AtomicBool},
	thread,
};

use inkflow_core::InkFlowEngine;

#[repr(i32)]
pub(crate) enum InkFlowStatus {
	Ok = 0,
	Null = 1,
	InvalidArgument = 2,
	InternalError = 3,
}

impl InkFlowStatus {
	pub(crate) fn code(self) -> i32 {
		self as i32
	}
}

pub(crate) type InkFlowUpdateCallback = Option<extern "C" fn(*const c_char, *mut c_void)>;

#[derive(Clone, Copy)]
pub(crate) struct UserData(pub(crate) *mut c_void);

unsafe impl Send for UserData {}

pub(crate) struct CallbackState {
	pub(crate) stop: Arc<AtomicBool>,
	pub(crate) thread: thread::JoinHandle<()>,
}

#[repr(C)]
pub struct InkFlowHandle {
	pub(crate) engine: Arc<Mutex<Option<InkFlowEngine>>>,
	pub(crate) callback: Mutex<Option<CallbackState>>,
}
