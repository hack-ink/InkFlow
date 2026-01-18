use std::{
	collections::VecDeque,
	time::{Duration, Instant},
};

use crate::{domain, settings::SttSettings, stt};

pub(crate) struct SegmentState {
	segment_id: u64,
	buffer: Vec<f32>,
	peak_mean_abs: f32,
}

pub(crate) struct WindowState {
	enabled: bool,
	step: Duration,
	emit_every: u64,
	window_len_16k_samples: usize,
	context_len_16k_samples: usize,
	window_ring: VecDeque<f32>,
	total_16k_samples: u64,
	window_generation: u64,
	window_job_id: u64,
	tick_index: u64,
	next_tick: Instant,
}

impl SegmentState {
	pub(crate) fn new() -> Self {
		Self { segment_id: 0, buffer: Vec::new(), peak_mean_abs: 0.0 }
	}

	pub(crate) fn push_samples(&mut self, samples: &[f32]) {
		self.buffer.extend_from_slice(samples);
		self.peak_mean_abs = self.peak_mean_abs.max(super::mean_abs(samples));
	}

	pub(crate) fn reset(&mut self) {
		self.buffer.clear();
		self.peak_mean_abs = 0.0;
	}

	pub(crate) fn is_empty(&self) -> bool {
		self.buffer.is_empty()
	}

	pub(crate) fn peak_mean_abs(&self) -> f32 {
		self.peak_mean_abs
	}

	pub(crate) fn next_segment_id(&mut self) -> u64 {
		self.segment_id = self.segment_id.saturating_add(1);
		self.segment_id
	}

	pub(crate) fn take(&mut self) -> (Vec<f32>, f32) {
		let samples = std::mem::take(&mut self.buffer);
		let peak = self.peak_mean_abs;
		self.peak_mean_abs = 0.0;
		(samples, peak)
	}
}

impl WindowState {
	pub(crate) fn new(settings: &SttSettings, enabled: bool) -> Self {
		let step = Duration::from_millis(settings.window.step_ms);
		let emit_every = settings.window.emit_every.max(1) as u64;
		let window_len_16k_samples = domain::ms_to_samples_16k(
			settings.window.window_ms.saturating_add(settings.window.context_ms),
		)
		.max(1) as usize;
		let context_len_16k_samples =
			domain::ms_to_samples_16k(settings.window.context_ms) as usize;

		Self {
			enabled,
			step,
			emit_every,
			window_len_16k_samples,
			context_len_16k_samples,
			window_ring: VecDeque::with_capacity(window_len_16k_samples),
			total_16k_samples: 0,
			window_generation: 0,
			window_job_id: 0,
			tick_index: 0,
			next_tick: Instant::now() + step,
		}
	}

	pub(crate) fn push_samples(&mut self, samples_16k: &[f32]) {
		if !self.enabled || samples_16k.is_empty() {
			return;
		}

		self.total_16k_samples = self.total_16k_samples.saturating_add(samples_16k.len() as u64);
		self.window_ring.extend(samples_16k.iter().copied());
		while self.window_ring.len() > self.window_len_16k_samples {
			self.window_ring.pop_front();
		}
	}

	pub(crate) fn drain_ready_jobs(
		&mut self,
		engine_generation: u64,
		should_emit: bool,
		allow_emit: bool,
	) -> Vec<(stt::WindowJobSnapshot, Vec<f32>)> {
		if !self.enabled || self.window_ring.is_empty() {
			return Vec::new();
		}

		let now = Instant::now();
		let mut jobs = Vec::new();
		while now >= self.next_tick {
			self.tick_index = self.tick_index.saturating_add(1);
			if allow_emit && should_emit && self.tick_index.is_multiple_of(self.emit_every) {
				self.window_job_id = self.window_job_id.saturating_add(1);
				let audio_16k: Vec<f32> = self.window_ring.iter().copied().collect();
				let snapshot = stt::WindowJobSnapshot {
					engine_generation,
					window_generation: self.window_generation,
					job_id: self.window_job_id,
					window_end_16k_samples: self.total_16k_samples,
					window_len_16k_samples: audio_16k.len(),
					context_len_16k_samples: self.context_len_16k_samples.min(audio_16k.len()),
				};
				jobs.push((snapshot, audio_16k));
			}
			self.next_tick += self.step;
		}

		jobs
	}

	pub(crate) fn advance_generation(&mut self) -> u64 {
		self.window_generation = self.window_generation.saturating_add(1);
		self.window_generation
	}

	pub(crate) fn total_16k_samples(&self) -> u64 {
		self.total_16k_samples
	}

	#[cfg(test)]
	pub(crate) fn ring_len(&self) -> usize {
		self.window_ring.len()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn segment_state_tracks_peak_and_reset() {
		let mut state = SegmentState::new();
		let samples = [0.1, -0.2, 0.05];
		state.push_samples(&samples);
		let expected = (0.1_f32.abs() + 0.2_f32.abs() + 0.05_f32.abs()) / 3.0;
		assert!((state.peak_mean_abs() - expected).abs() < 1e-6);
		assert!(!state.is_empty());
		state.reset();
		assert!(state.is_empty());
		assert_eq!(state.peak_mean_abs(), 0.0);
	}

	#[test]
	fn segment_state_allocates_ids_monotonically() {
		let mut state = SegmentState::new();
		let first = state.next_segment_id();
		let second = state.next_segment_id();
		assert!(second > first);
	}

	#[test]
	fn window_state_tracks_total_and_ring_len() {
		let settings = SttSettings::default();
		let mut state = WindowState::new(&settings, true);
		let samples = vec![0.1; 320];
		state.push_samples(&samples);
		assert_eq!(state.total_16k_samples(), samples.len() as u64);
		assert_eq!(state.ring_len(), samples.len());
	}

	#[test]
	fn window_state_advances_generation() {
		let settings = SttSettings::default();
		let mut state = WindowState::new(&settings, true);
		let g1 = state.advance_generation();
		let g2 = state.advance_generation();
		assert!(g2 > g1);
	}
}
