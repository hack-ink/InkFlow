use crate::{domain, engine::text, stt};

pub(super) fn window_tail_text(
	snapshot: &stt::WindowJobSnapshot,
	result: &stt::WhisperDecodeResult,
	committed_end_16k: u64,
) -> String {
	let window_start_16k =
		snapshot.window_end_16k_samples.saturating_sub(snapshot.window_len_16k_samples as u64);
	let mut out = String::new();

	for segment in &result.segments {
		let end = window_start_16k.saturating_add(domain::ms_to_samples_16k(segment.t1_ms));
		if end <= committed_end_16k {
			continue;
		}
		text::append_normalized(&mut out, &segment.text);
	}

	out
}

pub(super) fn render_preview(text: &str, max_chars: usize) -> String {
	if max_chars == 0 {
		return String::new();
	}

	let mut out = String::new();
	for (idx, ch) in text.chars().enumerate() {
		if idx >= max_chars {
			out.push('…');
			break;
		}
		out.push(ch);
	}
	out
}
