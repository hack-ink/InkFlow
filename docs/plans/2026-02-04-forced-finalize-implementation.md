# Forced Finalize on Explicit Stop Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Ensure explicit stop triggers a forced finalize so the last segment is committed, while keeping the existing fast-then-refine update order.

**Architecture:** Add a stream command channel to trigger forced finalize in the ASR worker, expose it through the engine and FFI, and update the macOS stop flow to call it before unregistering callbacks.

**Tech Stack:** Rust (`inkflow-core`, `inkflow-ffi`), Swift (macOS app), C FFI.

---

### Task 1: Add a Small Forced-Finalize Helper with Tests

**Files:**
- Modify: `packages/inkflow-core/src/engine/worker/stream.rs`
- Test: `packages/inkflow-core/src/engine/worker/stream.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn forced_finalize_fallback_text_uses_last_text_when_final_empty() {
	let resolved = forced_finalize_fallback_text("", "hello");
	assert_eq!(resolved, "hello");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p inkflow-core forced_finalize_fallback_text_uses_last_text_when_final_empty -v`  
Expected: FAIL with "cannot find function `forced_finalize_fallback_text`".

**Step 3: Write minimal implementation**

```rust
fn forced_finalize_fallback_text(final_text: &str, last_text: &str) -> String {
	let trimmed = final_text.trim();
	if trimmed.is_empty() {
		last_text.trim().to_string()
	} else {
		trimmed.to_string()
	}
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p inkflow-core forced_finalize_fallback_text_uses_last_text_when_final_empty -v`  
Expected: PASS.

**Step 5: Commit**

```bash
git add packages/inkflow-core/src/engine/worker/stream.rs
git commit -m "{\"schema\":\"cmsg/1\",\"type\":\"refactor\",\"scope\":\"engine\",\"summary\":\"Add forced finalize helper\",\"intent\":\"Centralize forced finalize text fallback\",\"impact\":\"Enables targeted tests for finalize logic\",\"breaking\":false,\"risk\":\"low\",\"refs\":[\"gh:hack-ink/InkFlow#11\"]}"
```

---

### Task 2: Add StreamCommand and Forced Finalize in the Core Pipeline

**Files:**
- Modify: `packages/inkflow-core/src/engine/worker/stream.rs`
- Modify: `packages/inkflow-core/src/engine/worker/mod.rs`
- Modify: `packages/inkflow-core/src/engine/pipeline.rs`
- Modify: `packages/inkflow-core/src/engine.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn forced_finalize_emits_endpoint_reset_when_no_audio() {
	let outcome = forced_finalize_should_emit_segment(false, "");
	assert!(!outcome);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p inkflow-core forced_finalize_emits_endpoint_reset_when_no_audio -v`  
Expected: FAIL with "cannot find function `forced_finalize_should_emit_segment`".

**Step 3: Write minimal implementation**

- Introduce `StreamCommand` enum (`Audio(Vec<f32>)`, `Finalize`) in `stream.rs`.
- Update `StreamWorker` loop to handle `Finalize` by processing pending buffers and calling `force_finalize()`.
- Add `force_finalize()` to `StreamWorker` that:
  - Pads tail audio and calls `input_finished()`.
  - Decodes sherpa and resolves fallback text using the helper.
  - Emits `EndpointReset` when there is no valid speech or text.
  - Emits `SegmentEnd` and schedules second-pass when valid.
  - Resets `segment_state`, `last_text`, and `stream`.
- Update channel types from `mpsc::Sender<Vec<f32>>` to `mpsc::Sender<StreamCommand>`.
- Add `InkFlowEngine::force_finalize()` and `SttPipeline::force_finalize()` that send `StreamCommand::Finalize` when the ASR worker exists.

**Step 4: Run test to verify it passes**

Run: `cargo test -p inkflow-core forced_finalize_emits_endpoint_reset_when_no_audio -v`  
Expected: PASS.

**Step 5: Commit**

```bash
git add packages/inkflow-core/src/engine.rs \
  packages/inkflow-core/src/engine/pipeline.rs \
  packages/inkflow-core/src/engine/worker/mod.rs \
  packages/inkflow-core/src/engine/worker/stream.rs
git commit -m "{\"schema\":\"cmsg/1\",\"type\":\"feat\",\"scope\":\"engine\",\"summary\":\"Add forced finalize control path\",\"intent\":\"Allow explicit stop to flush buffered speech\",\"impact\":\"Prevents last segment loss on stop\",\"breaking\":false,\"risk\":\"medium\",\"refs\":[\"gh:hack-ink/InkFlow#11\"]}"
```

---

### Task 3: Expose Forced Finalize Through FFI

**Files:**
- Modify: `packages/inkflow-ffi/src/api.rs`
- Modify: `packages/inkflow-ffi/include/inkflow.h`

**Step 1: Write the failing test**

```rust
#[test]
fn force_finalize_returns_ok_when_engine_missing() {
	assert_eq!(inkflow_engine_force_finalize(std::ptr::null_mut()), INKFLOW_ERR_NULL);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p inkflow-ffi force_finalize_returns_ok_when_engine_missing -v`  
Expected: FAIL with "cannot find function `inkflow_engine_force_finalize`".

**Step 3: Write minimal implementation**

- Add `inkflow_engine_force_finalize` to `api.rs`, map errors to `InkFlowStatus`.
- Add the declaration to `inkflow.h`.

**Step 4: Run test to verify it passes**

Run: `cargo test -p inkflow-ffi force_finalize_returns_ok_when_engine_missing -v`  
Expected: PASS.

**Step 5: Commit**

```bash
git add packages/inkflow-ffi/src/api.rs packages/inkflow-ffi/include/inkflow.h
git commit -m "{\"schema\":\"cmsg/1\",\"type\":\"feat\",\"scope\":\"ffi\",\"summary\":\"Expose forced finalize API\",\"intent\":\"Allow clients to flush buffered speech\",\"impact\":\"Enables explicit stop to finalize segments\",\"breaking\":false,\"risk\":\"low\",\"refs\":[\"gh:hack-ink/InkFlow#11\"]}"
```

---

### Task 4: Call Forced Finalize in the macOS Stop Flow

**Files:**
- Modify: `apps/macos/InkFlow/InkFlow/InkFlowClient.swift`
- Modify: `apps/macos/InkFlow/InkFlow/InkFlowViewModel.swift`

**Step 1: Write the failing test**

```swift
// Add a unit test if a Swift test target exists; otherwise skip and document.
```

**Step 2: Run test to verify it fails**

Run: `xcodebuild -scheme InkFlow test`  
Expected: FAIL if a test target is added; otherwise skip and note.

**Step 3: Write minimal implementation**

- Add `forceFinalize()` to `InkFlowClient` calling `inkflow_engine_force_finalize`.
- Update `InkFlowViewModel.stop()`:
  - Stop audio capture immediately.
  - Call `client?.forceFinalize()`.
  - If the call fails, unregister updates immediately.
  - If it succeeds, set a `pendingFinalize` flag and defer `unregisterUpdates()` until receiving `segment_end`, `endpoint_reset`, or `error`.
- Update `handleUpdate(_:)` to clear `pendingFinalize` and unregister when the finalize completion signal arrives.

**Step 4: Run test to verify it passes**

Run: `xcodebuild -scheme InkFlow test`  
Expected: PASS if a test target is added; otherwise skip and note.

**Step 5: Commit**

```bash
git add apps/macos/InkFlow/InkFlow/InkFlowClient.swift \
  apps/macos/InkFlow/InkFlow/InkFlowViewModel.swift
git commit -m "{\"schema\":\"cmsg/1\",\"type\":\"feat\",\"scope\":\"macos\",\"summary\":\"Finalize on explicit stop\",\"intent\":\"Trigger forced finalize before unregistering callbacks\",\"impact\":\"Prevents last segment loss in the UI\",\"breaking\":false,\"risk\":\"low\",\"refs\":[\"gh:hack-ink/InkFlow#11\"]}"
```

---

### Task 5: Full Verification

**Files:**
- None

**Step 1: Run Rust tests**

Run: `cargo test`  
Expected: PASS.

**Step 2: Document manual validation**

Manual: Start dictation, speak a short phrase, stop explicitly, confirm the final segment appears and can be refined.

**Step 3: Commit (if needed)**

```bash
git status --short
```

