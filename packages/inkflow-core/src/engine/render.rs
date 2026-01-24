use std::{
	collections::HashMap,
	time::{Duration, Instant},
};

use crate::{
	AsrUpdate, domain,
	settings::SttSettings,
	stt,
};

use super::text;

pub(crate) struct RenderState {
	merge: domain::MergeState,
	last_window_text: String,
	last_window_end_16k: u64,
	committed_end_16k: u64,
	current_window_generation: u64,
	last_window_update: Option<Instant>,
	last_sherpa_partial: String,
	live_tail: String,
	committed_segments: Vec<String>,
	segment_index: HashMap<u64, usize>,
	last_display_text: String,
	has_window_output: bool,
	last_second_pass_text: HashMap<u64, String>,
	window_tick: u64,
	last_emit_tick: u64,
	last_stable_len: usize,
}

impl RenderState {
	pub(crate) fn new() -> Self {
		Self {
			merge: domain::MergeState::default(),
			last_window_text: String::new(),
			last_window_end_16k: 0,
			committed_end_16k: 0,
			current_window_generation: 0,
			last_window_update: None,
			last_sherpa_partial: String::new(),
			live_tail: String::new(),
			committed_segments: Vec::new(),
			segment_index: HashMap::new(),
			last_display_text: String::new(),
			has_window_output: false,
			last_second_pass_text: HashMap::new(),
			window_tick: 0,
			last_emit_tick: 0,
			last_stable_len: 0,
		}
	}

	pub(crate) fn handle_update(
		&mut self,
		update: &AsrUpdate,
		settings: &SttSettings,
	) -> Option<AsrUpdate> {
		match update {
			AsrUpdate::SherpaPartial(text) => {
				self.last_sherpa_partial = text.clone();
				if self.has_window_output {
					return None;
				}
				if self.window_is_stale(settings) {
					let cleaned = text.trim();
					if cleaned.is_empty() {
						return None;
					}
					if cleaned != self.live_tail {
						self.live_tail = cleaned.to_string();
						return self.emit_render("sherpa_partial");
					}
				}
			},
			AsrUpdate::WindowResult { snapshot, result } => {
				if self.should_drop_window(snapshot) {
					return None;
				}

				let tail =
					window_tail_text(snapshot, result, self.committed_end_16k);
				if tail.is_empty() {
					return None;
				}

				let mode = domain::token_mode_for_language("auto", &tail);
				let deduped = domain::dedup_tail(
					&self.last_window_text,
					&tail,
					mode,
					settings.merge.overlap_k_words as usize,
					settings.merge.overlap_k_chars as usize,
				);
				let merged = self.merge.apply_candidate(
					&deduped,
					mode,
					settings.merge.stable_ticks as usize,
					settings.merge.rollback_threshold_tokens as usize,
				);

				self.window_tick = self.window_tick.saturating_add(1);
				let stable_len = self.merge.stable_len();
				let stable_advanced = stable_len > self.last_stable_len;
				self.last_stable_len = stable_len;
				let interval = settings.merge.stable_ticks.max(1) as u64;
				let emit_due = self.last_emit_tick == 0
					|| stable_advanced
					|| self.window_tick.saturating_sub(self.last_emit_tick) >= interval;

				self.last_window_text = deduped;
				self.last_window_end_16k = snapshot.window_end_16k_samples;
				self.last_window_update = Some(Instant::now());
				self.has_window_output = true;

				if !emit_due {
					return None;
				}
				self.last_emit_tick = self.window_tick;

				if merged.trim().is_empty() {
					return None;
				}
				self.live_tail = merged;
				return self.emit_render("window_result");
			},
			AsrUpdate::SegmentEnd {
				segment_id,
				sherpa_text,
				committed_end_16k_samples,
				window_generation_after,
				..
			} => {
				let live_tail = self.live_tail.trim();
				let provisional = if self.has_window_output && !live_tail.is_empty() {
					live_tail.to_string()
				} else {
					domain::choose_segment_provisional_text(
						sherpa_text,
						&self.live_tail,
						self.has_window_output,
					)
				};
				let provisional = provisional.trim();
				if !provisional.is_empty() {
					self.upsert_segment(*segment_id, provisional.to_string());
				}

				self.committed_end_16k = *committed_end_16k_samples;
				self.current_window_generation = *window_generation_after;
				self.last_window_end_16k = *committed_end_16k_samples;
				self.merge.reset();
				self.last_window_text.clear();
				self.last_sherpa_partial.clear();
				self.last_window_update = None;
				self.live_tail.clear();
				self.has_window_output = false;
				self.window_tick = 0;
				self.last_emit_tick = 0;
				self.last_stable_len = 0;
				return self.emit_render("segment_end");
			},
			AsrUpdate::EndpointReset { window_generation_after } => {
				self.current_window_generation = *window_generation_after;
				self.merge.reset();
				self.last_window_text.clear();
				self.last_sherpa_partial.clear();
				self.last_window_update = None;
				self.live_tail.clear();
				self.has_window_output = false;
				self.window_tick = 0;
				self.last_emit_tick = 0;
				self.last_stable_len = 0;
				return self.emit_render("endpoint_reset");
			},
			AsrUpdate::SecondPass { segment_id, text } => {
				let incoming = text.trim();
				if incoming.is_empty() {
					return None;
				}
				let Some(&index) = self.segment_index.get(segment_id) else {
					return None;
				};
				if let Some(existing) = self.last_second_pass_text.get(segment_id) {
					if existing == incoming {
						return None;
					}
				}
				let current = self.committed_segments.get(index).map(String::as_str).unwrap_or("");
				let mode = domain::token_mode_for_language("auto", incoming);
				if !domain::should_accept_second_pass_replacement(current, incoming, mode) {
					return None;
				}
				self.committed_segments[index] = incoming.to_string();
				self.last_second_pass_text.insert(*segment_id, incoming.to_string());
				return self.emit_render("second_pass");
			},
			_ => {},
		}

		None
	}

	pub(crate) fn should_forward(&self, update: &AsrUpdate) -> bool {
		match update {
			AsrUpdate::WindowResult { snapshot, .. } => !self.should_drop_window(snapshot),
			_ => true,
		}
	}

	fn should_drop_window(&self, snapshot: &stt::WindowJobSnapshot) -> bool {
		if snapshot.window_generation < self.current_window_generation {
			return true;
		}

		snapshot.window_end_16k_samples < self.last_window_end_16k
	}

	fn window_is_stale(&self, settings: &SttSettings) -> bool {
		let freshness_ms = settings.window.step_ms.saturating_mul(2);
		let freshness = Duration::from_millis(freshness_ms.max(1));
		let Some(last_window) = self.last_window_update else {
			return true;
		};

		last_window.elapsed() >= freshness
	}

	fn upsert_segment(&mut self, segment_id: u64, text: String) {
		if let Some(&index) = self.segment_index.get(&segment_id) {
			if let Some(slot) = self.committed_segments.get_mut(index) {
				*slot = text;
			}
			return;
		}

		let index = self.committed_segments.len();
		self.committed_segments.push(text);
		self.segment_index.insert(segment_id, index);
	}

	fn render_if_changed(&mut self) -> Option<AsrUpdate> {
		let display = self.compose_display_text();
		if display == self.last_display_text {
			return None;
		}
		self.last_display_text = display.clone();
		Some(AsrUpdate::LiveRender { text: display })
	}

	fn emit_render(&mut self, source: &'static str) -> Option<AsrUpdate> {
		let update = self.render_if_changed();
		if let Some(AsrUpdate::LiveRender { text }) = update.as_ref() {
			let preview = render_preview(text, 120);
			tracing::debug!(
				source,
				text_len = text.len(),
				preview = %preview,
				"Render update emitted."
			);
		}
		update
	}

	fn compose_display_text(&self) -> String {
		let mut out = String::new();
		for segment in &self.committed_segments {
			text::append_normalized(&mut out, segment);
		}
		text::append_normalized(&mut out, &self.live_tail);
		out
	}
}

fn window_tail_text(
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

fn render_preview(text: &str, max_chars: usize) -> String {
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

#[cfg(test)]
mod tests {
	use super::RenderState;
	use crate::{
		AsrUpdate, settings::SttSettings, stt,
		stt::WhisperDecodedSegment,
	};

	#[derive(Debug, Default)]
	struct JitterMetrics {
		updates: usize,
		tail_rewrites: usize,
		max_backtrack_chars: usize,
	}

	fn analyze_jitter(texts: &[String]) -> JitterMetrics {
		let mut metrics = JitterMetrics::default();
		if texts.len() < 2 {
			return metrics;
		}

		for window in texts.windows(2) {
			let [prev, next] = window else {
				continue;
			};
			metrics.updates += 1;
			let lcp = prev.chars().zip(next.chars()).take_while(|(a, b)| a == b).count();
			if lcp < prev.len() {
				let backtrack = prev.len().saturating_sub(lcp);
				if backtrack > 0 {
					metrics.tail_rewrites += 1;
					metrics.max_backtrack_chars = metrics.max_backtrack_chars.max(backtrack);
				}
			}
		}

		metrics
	}

	fn collect_render_updates(
		state: &mut RenderState,
		updates: &[AsrUpdate],
		settings: &SttSettings,
	) -> Vec<String> {
		let mut out = Vec::new();
		for update in updates {
			if let Some(AsrUpdate::LiveRender { text }) = state.handle_update(update, settings) {
				out.push(text);
			}
		}
		out
	}

	#[test]
	fn drops_stale_window_generations() {
		let mut state = RenderState::new();
		let settings = SttSettings::default();
		let snapshot = stt::WindowJobSnapshot {
			engine_generation: 1,
			window_generation: 0,
			job_id: 1,
			window_end_16k_samples: 1600,
			window_len_16k_samples: 1600,
			context_len_16k_samples: 0,
		};

		state.handle_update(
			&AsrUpdate::SegmentEnd {
				segment_id: 1,
				sherpa_text: "hello".into(),
				committed_end_16k_samples: 1600,
				window_generation_after: 2,
			},
			&settings,
		);

		let result = stt::WhisperDecodeResult {
			text: "hello".into(),
			segments: vec![WhisperDecodedSegment { t0_ms: 0, t1_ms: 100, text: "hello".into() }],
			has_timestamps: true,
		};

		let out =
			state.handle_update(&AsrUpdate::WindowResult { snapshot, result }, &settings);
		assert!(out.is_none());
	}

	#[test]
	fn stabilizes_after_multiple_matches() {
		let mut state = RenderState::new();
		let settings = SttSettings::default();
		let snapshot = stt::WindowJobSnapshot {
			engine_generation: 1,
			window_generation: 0,
			job_id: 1,
			window_end_16k_samples: 3200,
			window_len_16k_samples: 3200,
			context_len_16k_samples: 0,
		};
		let result = stt::WhisperDecodeResult {
			text: "hello world".into(),
			segments: vec![WhisperDecodedSegment { t0_ms: 0, t1_ms: 200, text: "hello".into() }],
			has_timestamps: true,
		};

		let mut out = None;
		for _ in 0..settings.merge.stable_ticks {
			out = state.handle_update(
				&AsrUpdate::WindowResult { snapshot: snapshot.clone(), result: result.clone() },
				&settings,
			);
		}

		let AsrUpdate::LiveRender { text } = out.expect("live render update expected") else {
			panic!("expected live render update");
		};
		assert!(!text.is_empty());
	}

	#[test]
	fn segment_end_emits_display_text() {
		let mut state = RenderState::new();
		let settings = SttSettings::default();

		let out = state.handle_update(
			&AsrUpdate::SegmentEnd {
				segment_id: 1,
				sherpa_text: "hello world".into(),
				committed_end_16k_samples: 1600,
				window_generation_after: 1,
			},
			&settings,
		);

		let AsrUpdate::LiveRender { text } = out.expect("live render update expected") else {
			panic!("expected live render update");
		};
		assert_eq!(text, "hello world");
	}

	#[test]
	fn duplicate_second_pass_updates_are_ignored() {
		let mut state = RenderState::new();
		let settings = SttSettings::default();

		let _ = state.handle_update(
			&AsrUpdate::SegmentEnd {
				segment_id: 1,
				sherpa_text: "hello world".into(),
				committed_end_16k_samples: 1600,
				window_generation_after: 1,
			},
			&settings,
		);

		let out = state.handle_update(
			&AsrUpdate::SecondPass { segment_id: 1, text: "hello whisper".into() },
			&settings,
		);
		assert!(out.is_some());

		let out = state.handle_update(
			&AsrUpdate::SecondPass { segment_id: 1, text: "hello whisper".into() },
			&settings,
		);
		assert!(out.is_none());
	}

	#[test]
	fn segment_end_prefers_window_tail_when_available() {
		let mut state = RenderState::new();
		let settings = SttSettings::default();
		let snapshot = stt::WindowJobSnapshot {
			engine_generation: 1,
			window_generation: 0,
			job_id: 1,
			window_end_16k_samples: 3200,
			window_len_16k_samples: 3200,
			context_len_16k_samples: 0,
		};
		let result = stt::WhisperDecodeResult {
			text: "hello window".into(),
			segments: vec![WhisperDecodedSegment { t0_ms: 0, t1_ms: 200, text: "hello window".into() }],
			has_timestamps: true,
		};

		let out = state.handle_update(
			&AsrUpdate::WindowResult { snapshot, result },
			&settings,
		);
		assert!(out.is_some());

		let out = state.handle_update(
			&AsrUpdate::SegmentEnd {
				segment_id: 1,
				sherpa_text: "GOODBYE".into(),
				committed_end_16k_samples: 3200,
				window_generation_after: 1,
			},
			&settings,
		);
		let display = match out {
			Some(AsrUpdate::LiveRender { text }) => text,
			None => state.compose_display_text(),
			_ => panic!("expected live render update or no update"),
		};
		assert!(display.contains("hello window"));
	}

	#[test]
	fn window_updates_are_throttled_until_interval() {
		let mut state = RenderState::new();
		let settings = SttSettings::default();
		let snapshot = stt::WindowJobSnapshot {
			engine_generation: 1,
			window_generation: 0,
			job_id: 1,
			window_end_16k_samples: 3200,
			window_len_16k_samples: 3200,
			context_len_16k_samples: 0,
		};

		let mut outputs = Vec::new();
		for text in ["hello", "hello a", "hello ab"] {
			let result = stt::WhisperDecodeResult {
				text: text.to_string(),
				segments: vec![WhisperDecodedSegment {
					t0_ms: 0,
					t1_ms: 200,
					text: text.to_string(),
				}],
				has_timestamps: true,
			};
			outputs.push(state.handle_update(
				&AsrUpdate::WindowResult { snapshot: snapshot.clone(), result },
				&settings,
			));
		}

		assert!(outputs[0].is_some());
		assert!(outputs[1].is_none());
		assert!(outputs[2].is_none());
	}

	#[test]
	fn window_shrink_without_stable_advance_is_ignored() {
		let mut state = RenderState::new();
		let mut settings = SttSettings::default();
		settings.merge.stable_ticks = 1;
		let snapshot = stt::WindowJobSnapshot {
			engine_generation: 1,
			window_generation: 0,
			job_id: 1,
			window_end_16k_samples: 3200,
			window_len_16k_samples: 3200,
			context_len_16k_samples: 0,
		};

		let first = stt::WhisperDecodeResult {
			text: "hi how are you".to_string(),
			segments: vec![WhisperDecodedSegment {
				t0_ms: 0,
				t1_ms: 200,
				text: "hi how are you".to_string(),
			}],
			has_timestamps: true,
		};
		let second = stt::WhisperDecodeResult {
			text: "hi how".to_string(),
			segments: vec![WhisperDecodedSegment {
				t0_ms: 0,
				t1_ms: 200,
				text: "hi how".to_string(),
			}],
			has_timestamps: true,
		};

		let out = state.handle_update(
			&AsrUpdate::WindowResult { snapshot: snapshot.clone(), result: first },
			&settings,
		);
		assert!(out.is_some());

		let out = state.handle_update(
			&AsrUpdate::WindowResult { snapshot, result: second },
			&settings,
		);
		assert!(out.is_none());
	}

	#[test]
	fn committed_prefix_is_preserved_across_window_updates() {
		let mut state = RenderState::new();
		let settings = SttSettings::default();

		let _ = state.handle_update(
			&AsrUpdate::SegmentEnd {
				segment_id: 1,
				sherpa_text: "hello".into(),
				committed_end_16k_samples: 1600,
				window_generation_after: 1,
			},
			&settings,
		);

		let snapshot = stt::WindowJobSnapshot {
			engine_generation: 1,
			window_generation: 1,
			job_id: 1,
			window_end_16k_samples: 3200,
			window_len_16k_samples: 3200,
			context_len_16k_samples: 0,
		};

		let updates = [
			AsrUpdate::WindowResult {
				snapshot: snapshot.clone(),
				result: stt::WhisperDecodeResult {
					text: "hello one two".into(),
					segments: vec![WhisperDecodedSegment {
						t0_ms: 0,
						t1_ms: 200,
						text: "hello one two".into(),
					}],
					has_timestamps: true,
				},
			},
			AsrUpdate::WindowResult {
				snapshot,
				result: stt::WhisperDecodeResult {
					text: "hello one two three".into(),
					segments: vec![WhisperDecodedSegment {
						t0_ms: 0,
						t1_ms: 200,
						text: "hello one two three".into(),
					}],
					has_timestamps: true,
				},
			},
		];

		let renders = collect_render_updates(&mut state, &updates, &settings);
		let latest = renders.last().expect("expected live render update");
		assert!(latest.starts_with("hello"));
	}

	#[ignore]
	#[test]
	fn jitter_probe_emits_metrics_for_window_tail_rewrites() {
		let mut state = RenderState::new();
		let settings = SttSettings::default();

		let snapshot = stt::WindowJobSnapshot {
			engine_generation: 1,
			window_generation: 0,
			job_id: 1,
			window_end_16k_samples: 3200,
			window_len_16k_samples: 3200,
			context_len_16k_samples: 0,
		};

		let updates = [
			AsrUpdate::WindowResult {
				snapshot: snapshot.clone(),
				result: stt::WhisperDecodeResult {
					text: "hello one two three".into(),
					segments: vec![WhisperDecodedSegment {
						t0_ms: 0,
						t1_ms: 200,
						text: "hello one two three".into(),
					}],
					has_timestamps: true,
				},
			},
			AsrUpdate::WindowResult {
				snapshot: snapshot.clone(),
				result: stt::WhisperDecodeResult {
					text: "hello one two three four".into(),
					segments: vec![WhisperDecodedSegment {
						t0_ms: 0,
						t1_ms: 200,
						text: "hello one two three four".into(),
					}],
					has_timestamps: true,
				},
			},
			AsrUpdate::WindowResult {
				snapshot,
				result: stt::WhisperDecodeResult {
					text: "hello one two free".into(),
					segments: vec![WhisperDecodedSegment {
						t0_ms: 0,
						t1_ms: 200,
						text: "hello one two free".into(),
					}],
					has_timestamps: true,
				},
			},
		];

		let renders = collect_render_updates(&mut state, &updates, &settings);
		let metrics = analyze_jitter(&renders);
		assert!(metrics.updates > 0);
		println!(
			"Jitter metrics: updates={}, tail_rewrites={}, max_backtrack_chars={}",
			metrics.updates, metrics.tail_rewrites, metrics.max_backtrack_chars
		);
	}
}
