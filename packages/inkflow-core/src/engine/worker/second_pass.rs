use super::audio::WhisperJob;

struct PendingSecondPass {
	segment_id: u64,
	sample_rate_hz: u32,
	samples: Vec<f32>,
	peak_mean_abs: f32,
	remaining_tail_samples: usize,
}

impl PendingSecondPass {
	fn append_tail(&mut self, samples: &[f32]) -> bool {
		if self.remaining_tail_samples == 0 || samples.is_empty() {
			return self.remaining_tail_samples == 0;
		}

		let take = self.remaining_tail_samples.min(samples.len());
		self.samples.extend_from_slice(&samples[..take]);
		self.remaining_tail_samples = self.remaining_tail_samples.saturating_sub(take);
		self.remaining_tail_samples == 0
	}

	fn into_job(self) -> WhisperJob {
		WhisperJob::SecondPass {
			segment_id: self.segment_id,
			sample_rate_hz: self.sample_rate_hz,
			samples: self.samples,
			peak_mean_abs: self.peak_mean_abs,
		}
	}
}

pub(crate) struct SecondPassScheduler {
	pending: Option<PendingSecondPass>,
}

impl SecondPassScheduler {
	pub(crate) fn new() -> Self {
		Self { pending: None }
	}

	pub(crate) fn schedule(
		&mut self,
		segment_id: u64,
		sample_rate_hz: u32,
		samples: Vec<f32>,
		peak_mean_abs: f32,
		tail_samples: usize,
	) -> Option<WhisperJob> {
		if tail_samples == 0 {
			return Some(WhisperJob::SecondPass {
				segment_id,
				sample_rate_hz,
				samples,
				peak_mean_abs,
			});
		}

		self.pending = Some(PendingSecondPass {
			segment_id,
			sample_rate_hz,
			samples,
			peak_mean_abs,
			remaining_tail_samples: tail_samples,
		});
		None
	}

	pub(crate) fn append_tail(&mut self, samples: &[f32]) -> Option<WhisperJob> {
		let mut pending = self.pending.take()?;

		if !pending.append_tail(samples) {
			self.pending = Some(pending);
			return None;
		}

		Some(pending.into_job())
	}

	pub(crate) fn flush(&mut self, force: bool) -> Option<WhisperJob> {
		let pending = self.pending.take()?;

		if pending.remaining_tail_samples == 0 || force {
			return Some(pending.into_job());
		}

		self.pending = Some(pending);
		None
	}
}

#[cfg(test)]
mod pending_tests {
	use super::PendingSecondPass;

	#[test]
	fn pending_second_pass_appends_tail_until_complete() {
		let mut pending = PendingSecondPass {
			segment_id: 1,
			sample_rate_hz: 16_000,
			samples: vec![0.1; 4],
			peak_mean_abs: 0.1,
			remaining_tail_samples: 4,
		};

		assert!(!pending.append_tail(&[0.2, 0.2]));
		assert_eq!(pending.remaining_tail_samples, 2);

		assert!(pending.append_tail(&[0.3, 0.3]));
		assert_eq!(pending.remaining_tail_samples, 0);
		assert_eq!(pending.samples.len(), 8);
	}
}

#[cfg(test)]
mod second_pass_scheduler_tests {
	use super::{SecondPassScheduler, WhisperJob};

	#[test]
	fn schedule_defers_until_tail_complete() {
		let mut scheduler = SecondPassScheduler::new();
		let scheduled = scheduler.schedule(1, 16_000, vec![0.1; 4], 0.2, 3);
		assert!(scheduled.is_none());

		let appended = scheduler.append_tail(&[0.2, 0.2]);
		assert!(appended.is_none());

		let appended = scheduler.append_tail(&[0.3]);
		let WhisperJob::SecondPass { samples, .. } =
			appended.expect("expected second-pass job after tail")
		else {
			panic!("expected second-pass job");
		};
		assert_eq!(samples.len(), 7);
	}

	#[test]
	fn flush_forces_enqueue_when_tail_remaining() {
		let mut scheduler = SecondPassScheduler::new();
		let scheduled = scheduler.schedule(2, 16_000, vec![0.1; 2], 0.2, 5);
		assert!(scheduled.is_none());

		let flushed = scheduler.flush(true);
		assert!(matches!(flushed, Some(WhisperJob::SecondPass { .. })));

		let appended = scheduler.append_tail(&[0.2, 0.2]);
		assert!(appended.is_none());
	}

	#[test]
	fn schedule_immediate_when_no_tail() {
		let mut scheduler = SecondPassScheduler::new();
		let scheduled = scheduler.schedule(3, 16_000, vec![0.1; 3], 0.2, 0);
		assert!(matches!(scheduled, Some(WhisperJob::SecondPass { .. })));
	}
}
