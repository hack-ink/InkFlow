use std::{
	collections::VecDeque,
	sync::{Condvar, Mutex},
	time::{Duration, Instant},
};

use super::worker::WhisperJob;

pub(crate) struct SecondPassQueue {
	capacity: usize,
	inner: Mutex<VecDeque<WhisperJob>>,
	available: Condvar,
	drop_log: Mutex<DropLogState>,
}

impl SecondPassQueue {
	pub(crate) fn new(capacity: usize) -> Self {
		Self {
			capacity: capacity.max(1),
			inner: Mutex::new(VecDeque::with_capacity(capacity.max(1))),
			available: Condvar::new(),
			drop_log: Mutex::new(DropLogState::new()),
		}
	}

	pub(crate) fn push(&self, job: WhisperJob) -> bool {
		let (accepted, drop_reason, drop_queue_len) = {
			let mut queue = self.inner.lock().unwrap_or_else(|err| err.into_inner());

			if queue.len() < self.capacity {
				queue.push_back(job);
				self.available.notify_one();
				return true;
			}

			let new_peak = peak_mean_abs(&job);
			let Some((drop_index, drop_peak)) = queue
				.iter()
				.enumerate()
				.map(|(idx, item)| (idx, peak_mean_abs(item)))
				.min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
			else {
				return false;
			};

			if new_peak <= drop_peak {
				(false, "incoming_low_energy", queue.len())
			} else {
				queue.remove(drop_index);
				queue.push_back(job);
				self.available.notify_one();
				(true, "replaced_low_energy", queue.len())
			}
		};

		self.record_drop(drop_reason, drop_queue_len);
		accepted
	}

	pub(crate) fn pop(&self, timeout: Duration) -> Option<WhisperJob> {
		let mut queue = self.inner.lock().unwrap_or_else(|err| err.into_inner());

		if queue.is_empty() {
			let (guard, _) =
				self.available.wait_timeout(queue, timeout).unwrap_or_else(|err| err.into_inner());
			queue = guard;
		}

		queue.pop_front()
	}

	pub(crate) fn len(&self) -> usize {
		let queue = self.inner.lock().unwrap_or_else(|err| err.into_inner());
		queue.len()
	}

	fn record_drop(&self, reason: &'static str, queue_len: usize) {
		let mut state = self.drop_log.lock().unwrap_or_else(|err| err.into_inner());
		state.dropped = state.dropped.saturating_add(1);

		let now = Instant::now();
		if now.duration_since(state.last_log) < state.interval {
			return;
		}

		tracing::warn!(
			dropped = state.dropped,
			queue_len,
			reason,
			"Second-pass queue dropped segments."
		);
		state.dropped = 0;
		state.last_log = now;
	}
}

fn peak_mean_abs(job: &WhisperJob) -> f32 {
	match job {
		WhisperJob::SecondPass { peak_mean_abs, .. } => *peak_mean_abs,
		WhisperJob::Window { .. } => 0.0,
	}
}

struct DropLogState {
	last_log: Instant,
	interval: Duration,
	dropped: u64,
}

impl DropLogState {
	fn new() -> Self {
		let now = Instant::now();
		let last_log = now.checked_sub(Duration::from_secs(60)).unwrap_or(now);
		Self { last_log, interval: Duration::from_secs(2), dropped: 0 }
	}
}

#[cfg(test)]
mod tests {
	use super::SecondPassQueue;
	use crate::engine::worker::WhisperJob;

	#[test]
	fn queue_drops_lowest_energy_when_full() {
		let queue = SecondPassQueue::new(2);
		assert!(queue.push(WhisperJob::SecondPass {
			segment_id: 1,
			sample_rate_hz: 16_000,
			samples: vec![0.0; 10],
			peak_mean_abs: 0.2,
		}));
		assert!(queue.push(WhisperJob::SecondPass {
			segment_id: 2,
			sample_rate_hz: 16_000,
			samples: vec![0.0; 10],
			peak_mean_abs: 0.4,
		}));
		assert!(queue.push(WhisperJob::SecondPass {
			segment_id: 3,
			sample_rate_hz: 16_000,
			samples: vec![0.0; 10],
			peak_mean_abs: 0.8,
		}));

		let first = queue.pop(std::time::Duration::from_millis(1)).unwrap();
		let second = queue.pop(std::time::Duration::from_millis(1)).unwrap();
		let ids = [first, second]
			.into_iter()
			.filter_map(|job| match job {
				WhisperJob::SecondPass { segment_id, .. } => Some(segment_id),
				_ => None,
			})
			.collect::<Vec<_>>();

		assert!(ids.contains(&2));
		assert!(ids.contains(&3));
	}
}
