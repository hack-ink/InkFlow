use crate::events::{
	ErrorEvent, LlmRewriteEvent, SessionStateEvent, SttFinalEvent, SttPartialEvent,
};

pub(crate) trait UiPort
where
	Self: Send + Sync,
{
	fn emit_session_state(&self, payload: SessionStateEvent);
	fn emit_stt_partial(&self, payload: SttPartialEvent);
	fn emit_stt_final(&self, payload: SttFinalEvent);
	fn emit_llm_rewrite(&self, payload: LlmRewriteEvent);
	fn emit_error(&self, payload: ErrorEvent);
	fn hide_overlay_window(&self);
	fn show_overlay_window(&self);
}
