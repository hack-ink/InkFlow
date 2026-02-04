# STT Backend Optimizations and Packages Rename Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement live text stabilization (A-F), dynamic backpressure, time-axis merge, caching reuse, and language-aware normalization; keep only local stream + local second pass; support English and Chinese input; update sherpa sys vendor header; rename the legacy crates directory to `packages/`; update relevant docs/spec.

**Architecture:** Introduce a render/merge state that consumes raw worker updates and emits a single live-render update with stable/unstable text. Add a bounded second-pass queue with drop policy, and dynamic backpressure to window emission. Upgrade activity gating with RMS, zero-crossing, and band-energy ratio. Add endpoint tail compensation via a pending tail buffer. Keep the decode mode as local stream + local second pass only. Update SwiftUI to use the new live-render update. Update docs/spec to reflect the new pipeline.

**Tech Stack:** Rust (tokio, std), SwiftUI, Markdown.

---

## Task 1: Rename the legacy crates directory to `packages/` and update paths

**Files:**
- Move: legacy crates directory -> `packages/`
- Modify: `Cargo.toml`
- Modify (if updated by Cargo): `Cargo.lock`
- Modify: `apps/macos/InkFlow/InkFlow.xcodeproj/project.pbxproj`
- Modify docs (see Task 9)

**Step 1: Rename the directory**

Run: `mv crates packages`  
Expected: `packages/` exists and the legacy crates directory is gone.

**Step 2: Update workspace members and path dependencies**

Edit `Cargo.toml`:

```toml
[workspace]
members = [
    "packages/inkflow-core",
    "packages/inkflow-ffi",
    "packages/sherpa-onnx",
    "packages/sherpa-onnx-sys",
    "research/stt_compare",
]

[workspace.dependencies]
sherpa-onnx     = { version = "0.1", path = "packages/sherpa-onnx" }
sherpa-onnx-sys = { version = "0.1", path = "packages/sherpa-onnx-sys" }
```

**Step 3: Update Xcode header search paths**

Replace the legacy header path with `packages/inkflow-ffi/include` in:
`apps/macos/InkFlow/InkFlow.xcodeproj/project.pbxproj`.

**Step 4: Verify path updates**

Run: `rg -n "legacy-crates-path" Cargo.toml apps/macos/InkFlow/InkFlow.xcodeproj/project.pbxproj`  
Expected: No matches.

---

## Task 2: Sync sherpa sys vendored header to upstream

**Files:**
- Modify: `packages/sherpa-onnx-sys/vendor/sherpa_onnx_c_api.h`
- Inspect: `packages/sherpa-onnx-sys/build.rs`

**Step 1: Replace the vendored header**

Run:  
`cp third_party/sherpa-onnx/sherpa-onnx/c-api/c-api.h packages/sherpa-onnx-sys/vendor/sherpa_onnx_c_api.h`

**Step 2: Verify build still succeeds**

Run: `cargo build -p sherpa-onnx-sys`  
Expected: Build succeeds and bindings generate.

---

## Task 3: Add settings for activity gating, backpressure, and endpoint tail

**Files:**
- Modify: `packages/inkflow-core/src/settings.rs`
- Test: `packages/inkflow-core/src/settings.rs`

**Step 1: Add new settings fields and defaults**

Add to `WhisperWindowSettings`:

```rust
pub struct WhisperWindowSettings {
    pub enabled: bool,
    pub window_ms: u64,
    pub step_ms: u64,
    pub context_ms: u64,
    pub min_mean_abs: f32,
    pub min_rms: f32,
    pub max_zero_crossing_rate: f32,
    pub min_band_energy_ratio: f32,
    pub emit_every: u32,
    pub endpoint_tail_ms: u64,
    pub window_backpressure_high_watermark: usize,
}
```

Defaults:

```rust
Self {
    enabled: true,
    window_ms: 4000,
    step_ms: 400,
    context_ms: 800,
    min_mean_abs: 0.001,
    min_rms: 0.001,
    max_zero_crossing_rate: 0.35,
    min_band_energy_ratio: 0.15,
    emit_every: 1,
    endpoint_tail_ms: 200,
    window_backpressure_high_watermark: 16,
}
```

**Step 2: Add second-pass queue settings**

Add to `SttSettings`:

```rust
pub struct SttSettings {
    pub sherpa: SherpaSettings,
    pub whisper: WhisperSettings,
    pub window: WhisperWindowSettings,
    pub merge: MergeSettings,
    pub profiles: WhisperProfiles,
    pub second_pass_queue_capacity: usize,
}
```

Default: `second_pass_queue_capacity: 16`.

**Step 3: Add validation**

```rust
if self.window.window_backpressure_high_watermark == 0 {
    return Err(AppError::new("settings_invalid", "window_backpressure_high_watermark must be greater than zero."));
}

if self.second_pass_queue_capacity == 0 {
    return Err(AppError::new("settings_invalid", "second_pass_queue_capacity must be greater than zero."));
}
```

**Step 4: Add tests**

Add tests for:

```rust
#[test]
fn window_backpressure_high_watermark_must_be_positive() {
    let mut settings = SttSettings::default();
    settings.window.window_backpressure_high_watermark = 0;
    assert!(settings.validate().is_err());
}

#[test]
fn second_pass_queue_capacity_must_be_positive() {
    let mut settings = SttSettings::default();
    settings.second_pass_queue_capacity = 0;
    assert!(settings.validate().is_err());
}
```

**Step 5: Run tests**

Run: `cargo test -p inkflow-core settings::`  
Expected: Tests fail before code changes, pass after.

---

## Task 4: Add language-aware text normalization and time-axis merge helpers

**Files:**
- Create: `packages/inkflow-core/src/engine/text.rs`
- Modify: `packages/inkflow-core/src/engine.rs`
- Test: `packages/inkflow-core/src/engine/text.rs`

**Step 1: Add helpers for CJK-aware spacing**

```rust
pub fn append_normalized(out: &mut String, next: &str) {
    let trimmed = next.trim();
    if trimmed.is_empty() {
        return;
    }
    if out.is_empty() {
        out.push_str(trimmed);
        return;
    }

    let last = out.chars().last().unwrap_or(' ');
    let next_first = trimmed.chars().next().unwrap_or(' ');
    let needs_space = needs_space_between(last, next_first);
    if needs_space {
        out.push(' ');
    }
    out.push_str(trimmed);
}
```

```rust
fn needs_space_between(left: char, right: char) -> bool {
    if is_cjk(left) || is_cjk(right) {
        return false;
    }
    left.is_ascii_alphanumeric() && right.is_ascii_alphanumeric()
}
```

**Step 2: Add CJK detection**

```rust
fn is_cjk(c: char) -> bool {
    let u = c as u32;
    matches!(
        u,
        0x4E00..=0x9FFF
            | 0x3400..=0x4DBF
            | 0x3040..=0x309F
            | 0x30A0..=0x30FF
            | 0xAC00..=0xD7AF
    )
}
```

**Step 3: Add tests**

```rust
#[test]
fn normalization_keeps_space_between_latin_words() {
    let mut out = String::from("hello");
    append_normalized(&mut out, "world");
    assert_eq!(out, "hello world");
}

#[test]
fn normalization_avoids_space_for_cjk() {
    let mut out = String::from("你好");
    append_normalized(&mut out, "世界");
    assert_eq!(out, "你好世界");
}
```

**Step 4: Wire module**

Add `mod text;` and export helpers from `engine.rs` as needed.

---

## Task 5: Add render/merge state for stable/unstable text and window dedup

**Files:**
- Create: `packages/inkflow-core/src/engine/render.rs`
- Modify: `packages/inkflow-core/src/engine.rs`
- Test: `packages/inkflow-core/src/engine/render.rs`

**Step 1: Define stable/unstable state**

```rust
pub struct RenderState {
    stable: String,
    unstable: String,
    stable_ticks: u32,
    consecutive_matches: u32,
    last_window_end_16k: u64,
    current_window_generation: u64,
    last_window_update: Option<std::time::Instant>,
    last_sherpa_partial: String,
}
```

**Step 2: Add window segment conversion with time-axis cutoff**

```rust
fn segments_to_tail_text(
    snapshot: &stt::WindowJobSnapshot,
    result: &stt::WhisperDecodeResult,
    committed_end_16k: u64,
) -> String {
    let window_start = snapshot.window_end_16k_samples.saturating_sub(snapshot.window_len_16k_samples as u64);
    let mut out = String::new();
    for seg in &result.segments {
        let start = window_start.saturating_add(domain::ms_to_samples_16k(seg.t0_ms));
        let end = window_start.saturating_add(domain::ms_to_samples_16k(seg.t1_ms));
        if end <= committed_end_16k {
            continue;
        }
        if end <= committed_end_16k {
            continue;
        }
        text::append_normalized(&mut out, &seg.text);
    }
    out
}
```

**Step 3: Stabilize tail with stable ticks**

```rust
fn update_stable(&mut self, new_tail: String, stable_ticks: u32) {
    if new_tail == self.unstable {
        self.consecutive_matches = self.consecutive_matches.saturating_add(1);
    } else {
        self.unstable = new_tail;
        self.consecutive_matches = 1;
    }

    if self.consecutive_matches >= stable_ticks {
        if !self.unstable.is_empty() {
            text::append_normalized(&mut self.stable, &self.unstable);
        }
        self.unstable.clear();
        self.consecutive_matches = 0;
    }
}
```

**Step 4: Add monotonic drop rule**

Drop window results if:
- `snapshot.window_generation < current_window_generation`, or
- `snapshot.window_end_16k_samples < last_window_end_16k`.

**Step 5: Add render choice**

Return `RenderUpdate::LiveText { text }` when:
- A window result arrives and passes the monotonic check, or
- A sherpa partial arrives and the last window update is older than `2 * window.step_ms`.

**Step 6: Tests**

Add tests for:
- Dropping stale window generations.
- Stable tick promotion after N matches.
- CJK spacing in stable/unstable concatenation.
- Sherpa fallback when window is stale.

**Step 7: Run tests**

Run: `cargo test -p inkflow-core engine::render`  
Expected: Tests fail before code, pass after.

---

## Task 6: Introduce bounded second-pass queue and dynamic backpressure

**Files:**
- Create: `packages/inkflow-core/src/engine/queue.rs`
- Modify: `packages/inkflow-core/src/engine/worker.rs`
- Modify: `packages/inkflow-core/src/engine/pipeline.rs`
- Test: `packages/inkflow-core/src/engine/queue.rs`

**Step 1: Implement bounded queue with drop policy**

```rust
pub struct SecondPassQueue {
    capacity: usize,
    inner: std::sync::Mutex<std::collections::VecDeque<WhisperJob>>,
    available: std::sync::Condvar,
}

impl SecondPassQueue {
    pub fn push(&self, job: WhisperJob) -> bool { /* drop policy */ }
    pub fn pop(&self, timeout: std::time::Duration) -> Option<WhisperJob> { /* blocking pop */ }
    pub fn len(&self) -> usize { /* current length */ }
}
```

Drop policy: when full, drop the job with the lowest `peak_mean_abs`; if the new job is lower or equal, drop the new job.

**Step 2: Tests**

Add tests to ensure:
- Push drops the lowest energy job when full.
- Pop returns jobs in FIFO order when no drops.

**Step 3: Wire queue into workers**

Replace `std::sync::mpsc::Sender<WhisperJob>` with `Arc<SecondPassQueue>` for second pass.

**Step 4: Add backpressure check**

In `StreamWorker`, if `second_pass_queue.len()` exceeds `window_backpressure_high_watermark`, skip window emissions for that tick.

**Step 5: Run tests**

Run: `cargo test -p inkflow-core engine::queue`  
Expected: Tests fail before code, pass after.

---

## Task 7: Upgrade activity gating (RMS, ZCR, band-energy)

**Files:**
- Modify: `packages/inkflow-core/src/engine/worker.rs`
- Test: `packages/inkflow-core/src/engine/worker.rs`

**Step 1: Add helper to compute activity stats**

```rust
fn activity_metrics(samples: &[f32], sample_rate_hz: u32) -> (f32, f32, f32) {
    let rms = /* compute RMS */;
    let zcr = /* zero crossing rate */;
    let band_ratio = /* simple band energy ratio */;
    (rms, zcr, band_ratio)
}
```

**Step 2: Gate window and second pass**

Require:
- `mean_abs >= min_mean_abs`
- `rms >= min_rms`
- `zcr <= max_zero_crossing_rate`
- `band_ratio >= min_band_energy_ratio`

**Step 3: Tests**

Add tests for:
- `activity_metrics` returns low RMS for silence.
- `zcr` increases with high-frequency toggling.

**Step 4: Run tests**

Run: `cargo test -p inkflow-core engine::worker`  
Expected: Tests fail before code, pass after.

---

## Task 8: Add endpoint tail compensation

**Files:**
- Modify: `packages/inkflow-core/src/engine/worker.rs`
- Test: `packages/inkflow-core/src/engine/worker.rs`

**Step 1: Add pending tail buffer**

```rust
struct PendingSecondPass {
    segment_id: u64,
    sample_rate_hz: u32,
    samples: Vec<f32>,
    peak_mean_abs: f32,
    remaining_tail_samples: usize,
}
```

When an endpoint triggers, store a `PendingSecondPass` and keep appending subsequent samples until `remaining_tail_samples == 0`, then enqueue the second-pass job.

**Step 2: Flush pending on finalize**

If the stream ends and a pending job remains, enqueue with whatever tail is available.

**Step 3: Tests**

Add tests to ensure:
- Tail samples are appended before enqueue.
- Pending job flushes on finalize.

**Step 4: Run tests**

Run: `cargo test -p inkflow-core engine::worker`  
Expected: Tests fail before code, pass after.

---

## Task 9: Update FFI + SwiftUI to use the new live-render update

**Files:**
- Modify: `packages/inkflow-core/src/engine.rs`
- Modify: `packages/inkflow-ffi/src/lib.rs`
- Modify: `apps/macos/InkFlow/InkFlow/InkFlowViewModel.swift`

**Step 1: Add a new update variant**

```rust
pub enum AsrUpdate {
    LiveRender { text: String },
    /* existing variants */
}
```

**Step 2: Emit LiveRender updates**

Wire `RenderState` into `SttPipeline::poll_update` so that `LiveRender` updates are emitted after processing `SherpaPartial` and `WindowResult`.

**Step 3: Update FFI JSON**

```rust
AsrUpdate::LiveRender { text } => json!({
    "kind": "live_render",
    "text": text,
}).to_string(),
```

**Step 4: Update SwiftUI**

Handle `live_render` by setting `liveText`, and ignore `sherpa_partial`/`window_result` for UI rendering.

---

## Task 10: Update docs/spec to match the new architecture

**Files:**
- Modify: `docs/spec/architecture.md`
- Modify: `docs/spec/core/speech_to_text.md`
- Modify: `docs/spec/core/stt_dictation_pipeline.md`
- Modify: `docs/spec/ui/backend_ui_contract.md`
- Modify: `docs/guide/development/setup_and_configuration.md`

**Step 1: Update path references** (legacy crates directory -> `packages/`)
**Step 2: Document LiveRender update flow**
**Step 3: Document backpressure and activity gating**
**Step 4: Document English/Chinese normalization**

---

## Task 11: Final verification

**Step 1: Ensure no stale legacy path references remain**

Run: `rg -n "legacy-crates-path" -S .`  
Expected: No matches.

**Step 2: Build core**

Run: `cargo build -p inkflow-core`  
Expected: Build succeeds.

---

**Notes:**
- Per user request, do not commit changes.
- Run tests only as specified above.
