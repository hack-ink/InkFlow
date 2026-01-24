# STT Orchestrator Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Align the STT backend and UI contract to the Orchestrator/Scheduler/Fusion architecture,
eliminate all-caps regressions, and keep the current local stream + local second-pass behavior.

**Architecture:** Introduce explicit Orchestrator, Scheduler, and Fusion modules. Emit a single
`RenderUpdate` to the UI. Keep decoders isolated and keep transcript semantics in Fusion.

**Tech Stack:** Rust (inkflow-core, inkflow-ffi), SwiftUI (InkFlow app), sherpa-onnx, whisper-rs,
tokio, tracing.

---

### Task 0: Stabilize existing logging and tooling changes

**Files:**
- Modify: `packages/inkflow-ffi/src/logging.rs`
- Modify: `packages/inkflow-ffi/src/lib.rs`
- Modify: `packages/inkflow-ffi/Cargo.toml`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Create: `scripts/run-debug.sh`

**Step 1: Run logging tests**

Run: `cargo test -p inkflow-ffi logging -- --nocapture`  
Expected: PASS.

**Step 2: Validate debug script behavior**

Run: `scripts/run-debug.sh`  
Expected: Builds and launches the Debug app bundle.

**Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock packages/inkflow-ffi/Cargo.toml \
  packages/inkflow-ffi/src/logging.rs packages/inkflow-ffi/src/lib.rs \
  scripts/run-debug.sh
git commit -m "feat: add file-only logging and debug run script"
```

### Task 0a: Restore build by removing incomplete recent-speech gate

**Files:**
- Modify: `packages/inkflow-core/src/engine/worker.rs`

**Step 1: Remove recent-speech hold logic**

Remove the `last_speech_at` field and the `has_recent_speech` method, plus any uses of them.
Window gating should fall back to `gate.allows(&metrics)` without cross-thread speech hold.

**Step 2: Run compile check**

Run: `cargo test -p inkflow-core activity_metrics_detects_silence -- --nocapture`  
Expected: PASS (confirms the crate builds).

**Step 3: Commit**

```bash
git add packages/inkflow-core/src/engine/worker.rs
git commit -m "fix: remove broken recent-speech window gate"
```

### Task 1: Update UI contract to RenderUpdate-only

**Files:**
- Modify: `docs/spec/ui/backend_ui_contract.md`
- Modify: `docs/spec/core/stt_orchestrator_architecture.md`

**Step 1: Draft contract changes**

Add a `render_update` section that defines `display_text` and optional `segments_delta`, and mark
`sherpa_partial`, `window_result`, and `second_pass` as diagnostic-only.

**Step 2: Save contract updates**

Write the updated sections in `docs/spec/ui/backend_ui_contract.md`.

**Step 3: Cross-link orchestration doc**

Ensure `docs/spec/core/stt_orchestrator_architecture.md` references the `render_update` contract.

**Step 4: Commit**

```bash
git add docs/spec/ui/backend_ui_contract.md docs/spec/core/stt_orchestrator_architecture.md
git commit -m "spec: define render_update-only UI contract"
```

### Task 2: Fusion fallback fix for all-caps regression

**Files:**
- Modify: `packages/inkflow-core/src/engine/render.rs`
- Test: `packages/inkflow-core/src/engine/render.rs` (unit tests in module)

**Step 1: Write failing tests**

Add tests that assert:
- When window is enabled and a window result has been seen, a later `SherpaPartial` does not emit
  a `LiveRender` fallback.
- When window is disabled, `SherpaPartial` still emits `LiveRender`.

**Step 2: Run test to verify failure**

Run: `cargo test -p inkflow-core render_state_ -- --nocapture`  
Expected: FAIL until behavior is implemented.

**Step 3: Implement minimal fix**

Track `has_window_output` in `RenderState` and only fallback to sherpa when:
- Window is disabled, or
- No window output has been observed for the current segment.

**Step 4: Run tests**

Run: `cargo test -p inkflow-core render_state_ -- --nocapture`  
Expected: PASS.

**Step 5: Commit**

```bash
git add packages/inkflow-core/src/engine/render.rs
git commit -m "fix: avoid sherpa fallback after window output"
```

### Task 3: Filter diagnostic updates in FFI

**Files:**
- Modify: `packages/inkflow-ffi/src/lib.rs`
- Modify: `docs/spec/ui/backend_ui_contract.md`

**Step 1: Write failing test**

Add a unit test to ensure the update mapping only emits `live_render` and `error` by default.

**Step 2: Run test to verify failure**

Run: `cargo test -p inkflow-ffi update_to_json -- --nocapture`  
Expected: FAIL until filtering is implemented.

**Step 3: Implement filtering**

Gate non-render events behind an explicit opt-in (for example, `INKFLOW_STT_DIAGNOSTICS=1`). When
unset, map only `live_render` and `error` to JSON payloads.

**Step 4: Run tests**

Run: `cargo test -p inkflow-ffi update_to_json -- --nocapture`  
Expected: PASS.

**Step 5: Commit**

```bash
git add packages/inkflow-ffi/src/lib.rs docs/spec/ui/backend_ui_contract.md
git commit -m "feat: filter diagnostic events from UI updates"
```

### Task 4: Update SwiftUI to consume RenderUpdate only

**Files:**
- Modify: `apps/macos/InkFlow/InkFlow/InkFlowViewModel.swift`

**Step 1: Update event handling**

Remove `segment_end` and `second_pass` handling and rely on `live_render` to update `transcript`.

**Step 2: Simplify transcript state**

Remove `segments` and `segmentIndex` if no longer needed, keeping only `liveText` and `transcript`.

**Step 3: Build the app**

Run: `scripts/run-debug.sh`  
Expected: Build succeeds and the UI updates from `live_render`.

**Step 4: Commit**

```bash
git add apps/macos/InkFlow/InkFlow/InkFlowViewModel.swift
git commit -m "refactor: render transcript from live_render only"
```

### Task 5: Orchestrator module naming and boundaries

**Files:**
- Modify: `packages/inkflow-core/src/engine.rs`
- Modify: `packages/inkflow-core/src/engine/pipeline.rs`
- Modify: `packages/inkflow-core/src/engine/render.rs`
- Create: `packages/inkflow-core/src/engine/orchestrator.rs`
- Create: `packages/inkflow-core/src/engine/scheduler.rs`
- Create: `packages/inkflow-core/src/engine/fusion.rs`

**Step 1: Create Orchestrator wrapper**

Move `SttPipeline` into `orchestrator.rs`, and rename it to `SttOrchestrator`.

**Step 2: Add Scheduler wrapper**

Introduce a `Scheduler` struct that owns window/backpressure decisions currently embedded in the
stream worker. Keep behavior identical by delegating to existing settings.

**Step 3: Add Fusion wrapper**

Wrap `RenderState` in a `FusionEngine` that exposes `handle_update` and `should_forward`.

**Step 4: Wire engine module**

Update `engine.rs` to use `SttOrchestrator` and `FusionEngine`, keeping public API stable.

**Step 5: Run core tests**

Run: `cargo test -p inkflow-core -- --nocapture`  
Expected: PASS.

**Step 6: Commit**

```bash
git add packages/inkflow-core/src/engine.rs \
  packages/inkflow-core/src/engine/orchestrator.rs \
  packages/inkflow-core/src/engine/scheduler.rs \
  packages/inkflow-core/src/engine/fusion.rs \
  packages/inkflow-core/src/engine/pipeline.rs \
  packages/inkflow-core/src/engine/render.rs
git commit -m "refactor: introduce orchestrator scheduler and fusion modules"
```

### Task 6: Update STT docs for contract and architecture

**Files:**
- Modify: `docs/spec/architecture.md`
- Modify: `docs/spec/core/speech_to_text.md`
- Modify: `docs/spec/core/stt_dictation_pipeline.md`

**Step 1: Align references**

Ensure docs refer to Orchestrator/Scheduler/Fusion and RenderUpdate-only UI contract.

**Step 2: Commit**

```bash
git add docs/spec/architecture.md docs/spec/core/speech_to_text.md docs/spec/core/stt_dictation_pipeline.md
git commit -m "docs: align STT specs with orchestrator contract"
```
