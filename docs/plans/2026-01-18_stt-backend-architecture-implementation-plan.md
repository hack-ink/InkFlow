# STT Backend Architecture Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refactor the backend STT pipeline to be modular, LLM-friendly, and extensible for future mode combinations, while keeping the current behavior (local stream + local second pass) and supporting English/Chinese input.

**Architecture:** Introduce a small mode router that yields a pipeline plan, plus a local-only pipeline implementation. Extract window and segment state into dedicated structs to reduce duplication and clarify responsibilities. Keep the hot path behavior equivalent to the existing pipeline.

**Tech Stack:** Rust (inkflow-core), tokio, sherpa-onnx, whisper-rs.

---

### Task 0: Work on main branch (per user request)

**Files:**
- None.

**Step 1: Run baseline build**

Run: `cargo build -p inkflow-core`

Expected: Build succeeds.

**Step 2: Run baseline tests**

Run: `cargo test -p inkflow-core`

Expected: Tests pass. If failures exist, report and ask how to proceed.

---

### Task 1: Default Whisper language to auto (English + Chinese support)

**Files:**
- Modify: `packages/inkflow-core/src/settings.rs`
- Test: `packages/inkflow-core/src/settings.rs`

**Step 1: Write the failing test**

Add to `packages/inkflow-core/src/settings.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::WhisperSettings;

    #[test]
    fn whisper_default_language_is_auto() {
        let settings = WhisperSettings::default();
        assert_eq!(settings.language, "auto");
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p inkflow-core whisper_default_language_is_auto`

Expected: FAIL because the default is still `en`.

**Step 3: Write minimal implementation**

Update the default in `WhisperSettings::default()`:

```rust
Self { model_path: String::new(), language: "auto".into(), num_threads: None, force_gpu: None }
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p inkflow-core whisper_default_language_is_auto`

Expected: PASS.

**Step 5: Commit (only if explicitly requested)**

```bash
git add packages/inkflow-core/src/settings.rs
git commit -m "feat: default whisper language to auto"
```

---

### Task 2: Add a mode router and pipeline plan (local stream + local second pass)

**Files:**
- Create: `packages/inkflow-core/src/engine/modes.rs`
- Modify: `packages/inkflow-core/src/engine.rs`
- Test: `packages/inkflow-core/src/engine/modes.rs`

**Step 1: Write the failing test**

Create `packages/inkflow-core/src/engine/modes.rs` with this test and stub types:

```rust
use crate::settings::SttSettings;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DecodeMode {
    StreamSecondPass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InferenceMode {
    LocalOnly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PipelinePlan {
    pub(crate) decode_mode: DecodeMode,
    pub(crate) inference: InferenceMode,
    pub(crate) window_enabled: bool,
}

pub(crate) struct ModeRouter;

impl ModeRouter {
    pub(crate) fn resolve(_settings: &SttSettings) -> PipelinePlan {
        todo!("Return the default plan");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_router_defaults_to_local_stream_second_pass() {
        let settings = SttSettings::default();
        let plan = ModeRouter::resolve(&settings);
        assert_eq!(plan.decode_mode, DecodeMode::StreamSecondPass);
        assert_eq!(plan.inference, InferenceMode::LocalOnly);
        assert_eq!(plan.window_enabled, settings.window.enabled);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p inkflow-core mode_router_defaults_to_local_stream_second_pass`

Expected: FAIL with panic from `todo!`.

**Step 3: Write minimal implementation**

Implement the router in `ModeRouter::resolve`:

```rust
PipelinePlan {
    decode_mode: DecodeMode::StreamSecondPass,
    inference: InferenceMode::LocalOnly,
    window_enabled: settings.window.enabled,
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p inkflow-core mode_router_defaults_to_local_stream_second_pass`

Expected: PASS.

**Step 5: Commit (only if explicitly requested)**

```bash
git add packages/inkflow-core/src/engine/modes.rs packages/inkflow-core/src/engine.rs
git commit -m "feat: add stt mode router plan"
```

---

### Task 3: Extract window and segment state structs for clarity

**Files:**
- Create: `packages/inkflow-core/src/engine/state.rs`
- Modify: `packages/inkflow-core/src/engine.rs`
- Test: `packages/inkflow-core/src/engine/state.rs`

**Step 1: Write the failing tests**

Create `packages/inkflow-core/src/engine/state.rs` with tests and stub types:

```rust
use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::{domain, settings::SttSettings};

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
        todo!("Implement SegmentState::new");
    }
    pub(crate) fn push_samples(&mut self, _samples: &[f32]) {
        todo!("Implement SegmentState::push_samples");
    }
    pub(crate) fn reset(&mut self) {
        todo!("Implement SegmentState::reset");
    }
    pub(crate) fn is_empty(&self) -> bool {
        todo!("Implement SegmentState::is_empty");
    }
    pub(crate) fn peak_mean_abs(&self) -> f32 {
        todo!("Implement SegmentState::peak_mean_abs");
    }
    pub(crate) fn next_segment_id(&mut self) -> u64 {
        todo!("Implement SegmentState::next_segment_id");
    }
    pub(crate) fn take(&mut self) -> (Vec<f32>, f32) {
        todo!("Implement SegmentState::take");
    }
}

impl WindowState {
    pub(crate) fn new(_settings: &SttSettings, _enabled: bool) -> Self {
        todo!("Implement WindowState::new");
    }
    pub(crate) fn push_samples(&mut self, _samples_16k: &[f32]) {
        todo!("Implement WindowState::push_samples");
    }
    pub(crate) fn advance_generation(&mut self) -> u64 {
        todo!("Implement WindowState::advance_generation");
    }
    pub(crate) fn total_16k_samples(&self) -> u64 {
        todo!("Implement WindowState::total_16k_samples");
    }
    pub(crate) fn ring_len(&self) -> usize {
        todo!("Implement WindowState::ring_len");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_state_tracks_peak_and_reset() {
        let mut state = SegmentState::new();
        state.push_samples(&[0.1, -0.2, 0.05]);
        assert!((state.peak_mean_abs() - 0.2).abs() < 1e-6);
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
```

**Step 2: Run tests to verify failure**

Run: `cargo test -p inkflow-core segment_state_tracks_peak_and_reset`

Expected: FAIL with panic from `todo!`.

**Step 3: Write minimal implementation**

Implement `SegmentState` and `WindowState` using the existing logic from the current engine:

```rust
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

    pub(crate) fn advance_generation(&mut self) -> u64 {
        self.window_generation = self.window_generation.saturating_add(1);
        self.window_generation
    }

    pub(crate) fn total_16k_samples(&self) -> u64 {
        self.total_16k_samples
    }

    pub(crate) fn ring_len(&self) -> usize {
        self.window_ring.len()
    }
}
```

**Step 4: Run tests to verify pass**

Run: `cargo test -p inkflow-core segment_state_tracks_peak_and_reset`

Expected: PASS.

**Step 5: Commit (only if explicitly requested)**

```bash
git add packages/inkflow-core/src/engine/state.rs packages/inkflow-core/src/engine.rs
git commit -m "refactor: extract window and segment state"
```

---

### Task 4: Refactor the engine to use the router and state structs

**Files:**
- Modify: `packages/inkflow-core/src/engine.rs`
- Modify: `packages/inkflow-core/src/lib.rs`

**Step 1: Write the failing test (router already failing if not integrated)**

No new test is required here, but the existing router and state tests must keep passing. This task is a structural refactor; the guardrail is `cargo test -p inkflow-core`.

**Step 2: Refactor structure**

Apply these changes:

- Add `mod modes;` and `mod state;` at the top of `engine.rs`.
- Replace the monolithic engine with:
  - `ModeRouter` usage to select a `PipelinePlan`.
  - A `SttPipeline` enum with a single variant for now (`LocalStreamSecondPass`).
  - A `LocalStreamSecondPassPipeline` struct that owns runtime, channels, and workers.
- Move window/segment buffering logic to `WindowState` and `SegmentState`.
- Extract endpoint handling into a dedicated method to avoid duplication between endpoint and finalize paths.
- Keep the Whisper worker and stream worker in dedicated structs to reduce cognitive load.
- Preserve current behavior (no new modes implemented yet).

**Step 3: Run tests to verify pass**

Run: `cargo test -p inkflow-core`

Expected: PASS.

**Step 4: Commit (only if explicitly requested)**

```bash
git add packages/inkflow-core/src/engine.rs packages/inkflow-core/src/lib.rs
git commit -m "refactor: modularize stt engine pipeline"
```

---

### Task 5: Final verification

**Files:**
- None.

**Step 1: Run the full crate tests**

Run: `cargo test -p inkflow-core`

Expected: PASS.
