# Logging Guidelines and Retention Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Define consistent logging levels, retain logs for seven days, and align runtime logging with the guideline.

**Architecture:** Centralize retention in the logging initializer, keep high-frequency diagnostic logs at debug, and document the log-level policy in a single guideline referenced by the guide index.

**Tech Stack:** Rust (tracing, tracing-subscriber, tracing-appender), SwiftUI (InkFlow), docs.

---

### Task 1: Add logging guideline

**Files:**
- Create: `docs/guide/development/logging_guidelines.md`
- Modify: `docs/guide/index.md`

**Step 1: Write the guideline**

Define info/warn/error/debug usage, include STT examples, and specify message conventions.

**Step 2: Link from guide index**

Add the new guideline to `docs/guide/index.md`.

**Step 3: Commit**

```bash
git add docs/guide/development/logging_guidelines.md docs/guide/index.md
git commit -m "docs: add logging guidelines"
```

### Task 2: Add log retention cleanup

**Files:**
- Modify: `packages/inkflow-ffi/src/logging.rs`

**Step 1: Write failing test**

Add a unit test that creates temp log files older/newer than seven days and asserts old files are deleted.

**Step 2: Run test to verify it fails**

Run: `cargo test -p inkflow-ffi logging -- --nocapture`  
Expected: FAIL until cleanup is implemented.

**Step 3: Implement cleanup**

Add a retention pass (seven days) after the log directory is resolved.

**Step 4: Run tests**

Run: `cargo test -p inkflow-ffi logging -- --nocapture`  
Expected: PASS.

**Step 5: Commit**

```bash
git add packages/inkflow-ffi/src/logging.rs
git commit -m "feat: prune logs older than seven days"
```

### Task 3: Align log levels to guideline

**Files:**
- Modify: `packages/inkflow-core/src/engine/worker.rs`
- Modify: `packages/inkflow-core/src/engine/render.rs`
- Modify: `packages/inkflow-core/src/engine/queue.rs` (if needed)

**Step 1: Update log levels**

Move high-frequency diagnostic logs to debug, keep warnings and errors unchanged.

**Step 2: Run core tests**

Run: `cargo test -p inkflow-core -- --nocapture`  
Expected: PASS.

**Step 3: Commit**

```bash
git add packages/inkflow-core/src/engine/worker.rs \
  packages/inkflow-core/src/engine/render.rs \
  packages/inkflow-core/src/engine/queue.rs
git commit -m "chore: align log levels with guideline"
```
