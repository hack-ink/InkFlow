use std::time::{Duration, Instant};

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
				if self.window_is_stale(settings) {
					return Some(AsrUpdate::LiveRender { text: text.clone() });
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

				self.last_window_text = deduped;
				self.last_window_end_16k = snapshot.window_end_16k_samples;
				self.last_window_update = Some(Instant::now());

				return Some(AsrUpdate::LiveRender { text: merged });
			},
			AsrUpdate::SegmentEnd {
				committed_end_16k_samples,
				window_generation_after,
				..
			} => {
				self.committed_end_16k = *committed_end_16k_samples;
				self.current_window_generation = *window_generation_after;
				self.last_window_end_16k = *committed_end_16k_samples;
				self.merge.reset();
				self.last_window_text.clear();
				self.last_sherpa_partial.clear();
				self.last_window_update = None;
			},
			AsrUpdate::EndpointReset { window_generation_after } => {
				self.current_window_generation = *window_generation_after;
				self.merge.reset();
				self.last_window_text.clear();
				self.last_sherpa_partial.clear();
				self.last_window_update = None;
				return Some(AsrUpdate::LiveRender { text: String::new() });
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

#[cfg(test)]
mod tests {
	use super::RenderState;
	use crate::{
		AsrUpdate, settings::SttSettings, stt,
		stt::WhisperDecodedSegment,
	};

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
}
