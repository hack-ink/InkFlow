use inkflow_core::AsrUpdate;
use serde_json::json;

pub(crate) fn update_to_json(update: AsrUpdate) -> String {
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

pub(crate) fn error_json(code: &str, message: &str) -> String {
	json!({"kind": "error", "code": code, "message": message}).to_string()
}
