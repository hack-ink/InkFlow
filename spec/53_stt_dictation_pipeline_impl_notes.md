# STT Dictation Pipeline v2 – Implementation Notes

Date: 2025-12-17

This document records the implementation work done to align AiR with `spec/52_stt_dictation_pipeline_spec.md`.
It is intended as a compact, developer-facing log that can be used to recover context in a future session.

## What Was Implemented

### Sliding-window Whisper refinement (live tail)

- Added a whisper sliding-window loop that runs during `Listening` and emits `stt/partial` updates using `strategy=sliding_window`.
- Scheduling is bounded and non-backlogging:
  - Window decode jobs are enqueued into a bounded `sync_channel` (capacity 2) via `try_send`, and ticks are dropped when full.
- Stale window result rejection:
  - Window results carry a `WindowJobSnapshot` (`job_id`, `window_generation`, `engine_generation`, and `window_end_16k_samples`).
  - Results are dropped if they are not for the current `window_generation`, not newer than the last applied job, or too far behind the latest scheduled job.
- Merge behavior:
  - Timestamp-aware tail extraction is used when whisper timestamps are available.
  - Boundary de-duplication uses language-aware token overlap (word tokens for whitespace languages; char tokens for CJK).
- Live stability mechanism:
  - A stable prefix is promoted only after `stable_ticks` consecutive accepted candidates.
  - Rollback occurs when divergence exceeds `rollback_threshold_tokens`.

Code anchors:
- `src-tauri/src/pipeline/dictation.rs`
- `src-tauri/src/application/session_actor.rs`

### Forced finalize correctness (last words)

- On `Enter` (key up), the dictation pipeline flushes sherpa and commits a final segment even without a natural endpoint when:
  - The segment audio is not near-silent, and
  - The segment duration is above a small threshold.
- Whisper second pass runs on that forced-final segment and replaces the provisional segment text in-place.

Code anchors:
- `src-tauri/src/pipeline/dictation.rs`
- `src-tauri/src/application/session_actor.rs`

### Engine hot reload and settings hot switching

- Added `EngineManager` to own the in-process sherpa and whisper instances and support reloads:
  - Engine reload is performed in-process (drop + recreate).
  - An `engine_generation` counter is bumped on reload to reject stale results.
  - `engine/state` events are emitted for `ready`, `reloading`, and `error`.
- Added a UI-invokable command that persists and applies settings:
  - `engine_apply_settings(patch)` persists to disk and triggers soft-apply or reload as needed.
  - Reload is rejected while dictation is `Listening` or `Finalizing`.

Code anchors:
- `src-tauri/src/engine.rs`
- `src-tauri/src/commands.rs`
- `src-tauri/src/events.rs`

### UI-first settings (STT)

- Expanded the persisted settings model to include `stt`:
  - sherpa (model dir, provider, threads, chunk_ms, endpoint rules)
  - whisper (model path, language, threads, GPU override)
  - sliding window (enabled, window/step/context, min_mean_abs, emit_every)
  - merge/stability (stable_ticks, rollback threshold, overlap K)
  - decode profiles (window best_of, second-pass best_of)
- Updated the UI settings panel to expose these knobs and apply via `engine_apply_settings`.
- UI listens to `engine/state` and shows engine status when idle.

Code anchors:
- `src-tauri/src/settings.rs`
- `ui/src/App.tsx`

## Key Behavior Details

- Sherpa feeding uses fixed-size decode slices based on `sherpa.chunk_ms`.
- Whisper window and second pass share the same silence gate (`min_mean_abs`).
- Once a whisper-window tail has been applied in a segment, sherpa partials no longer overwrite the live tail to reduce flicker.
- When an endpoint is committed, the provisional segment prefers the current live tail text (when available) to reduce visible regressions.
- Whisper window decoding is gated on recent audio energy (step-sized tail) to reduce hallucinations after speech ends.

## Verification

- Ran `cargo make fmt`, `cargo make clippy`, and `cargo make nextest`.
- Ran `cd ui && npm run build`.
- Ran `cargo make dev` to confirm the app builds and launches (terminated manually after startup).

## Diagnostics

- Enable structured STT tracing with `AIR_STT_TRACE=1` (writes to `tmp/stt_trace/`) or `AIR_STT_TRACE_DIR=...`.
- A finalized dictation session writes:
  - `stt_session_<SESSION_ID>.ndjson` (event trace).
  - `stt_session_<SESSION_ID>.wav` (recorded audio).
- Workflow notes: `spec/54_stt_dictation_pipeline_debugging.md`.

## Known Limitations / Future Work

- CJK tokenization uses Unicode scalar `char` tokens, not grapheme clusters.
- Timestamp-aware merge uses segment-level timestamps (not word-level alignment).
- Engine reload is blocked while listening; soft changes apply immediately, but reload changes require stopping dictation first.

## 2025-12-20 Refactor Notes

- Introduced a Session Actor that owns session state and routes pipeline updates through a command channel.
- Split dictation workers into `pipeline/dictation.rs` with a typed update stream.
- Added UI and platform ports with Tauri-backed adapters for event emission and injection.
- Kept the public session facade in `src-tauri/src/session.rs` while moving implementation to `application/`.
- Replaced backend `mod.rs` module files with flat `*.rs` modules while preserving public module names.

Code anchors:
- `src-tauri/src/application/session_actor.rs`
- `src-tauri/src/pipeline/dictation.rs`
- `src-tauri/src/adapters/ui.rs`

Validation: Not run for this refactor.
