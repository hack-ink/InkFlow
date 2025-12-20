use serde::Serialize;
use tauri::Emitter;

use crate::session::SessionState;

pub const SESSION_STATE: &str = "session/state";
pub const STT_PARTIAL: &str = "stt/partial";
pub const STT_FINAL: &str = "stt/final";
pub const LLM_REWRITE: &str = "llm/rewrite";
pub const ERROR: &str = "error";
pub const ENGINE_STATE: &str = "engine/state";

#[derive(Clone, Debug, Serialize)]
pub struct SessionStateEvent {
	pub session_id: String,
	pub state: SessionState,
	pub reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ErrorEvent {
	pub session_id: Option<String>,
	pub code: String,
	pub message: String,
	pub recoverable: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SttPartialEvent {
	pub session_id: String,
	pub revision: u64,
	pub text: String,
	pub strategy: SttStrategy,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum SttStrategy {
	VadChunk,
	SlidingWindow,
}

#[derive(Clone, Debug, Serialize)]
pub struct SttFinalEvent {
	pub session_id: String,
	pub text: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct LlmRewriteEvent {
	pub session_id: String,
	pub text: String,
	pub model: String,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EngineState {
	Ready,
	Reloading,
	Error,
}

#[derive(Clone, Debug, Serialize)]
pub struct EngineStateEvent {
	pub state: EngineState,
	pub reason: Option<String>,
}

pub fn emit_session_state(app: &tauri::AppHandle, payload: SessionStateEvent) {
	if let Err(err) = app.emit(SESSION_STATE, payload) {
		eprintln!("Failed to emit session/state event: {err}.");
	}
}

pub fn emit_stt_partial(app: &tauri::AppHandle, payload: SttPartialEvent) {
	if let Err(err) = app.emit(STT_PARTIAL, payload) {
		eprintln!("Failed to emit stt/partial event: {err}.");
	}
}

pub fn emit_stt_final(app: &tauri::AppHandle, payload: SttFinalEvent) {
	if let Err(err) = app.emit(STT_FINAL, payload) {
		eprintln!("Failed to emit stt/final event: {err}.");
	}
}

pub fn emit_llm_rewrite(app: &tauri::AppHandle, payload: LlmRewriteEvent) {
	if let Err(err) = app.emit(LLM_REWRITE, payload) {
		eprintln!("Failed to emit llm/rewrite event: {err}.");
	}
}

pub fn emit_error(app: &tauri::AppHandle, payload: ErrorEvent) {
	if let Err(err) = app.emit(ERROR, payload) {
		eprintln!("Failed to emit error event: {err}.");
	}
}

pub fn emit_engine_state(app: &tauri::AppHandle, payload: EngineStateEvent) {
	if let Err(err) = app.emit(ENGINE_STATE, payload) {
		eprintln!("Failed to emit engine/state event: {err}.");
	}
}
