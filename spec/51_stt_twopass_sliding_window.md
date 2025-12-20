# STT Architecture Spec: sherpa streaming + whisper sliding-window + whisper second pass

Status: Working document. It may drift over time.

This document specifies the intended dictation pipeline for AiR that combines:

- sherpa-onnx streaming for low-latency partials and endpoint detection.
- whisper sliding-window decoding to continuously refine the live transcript while speaking.
- whisper second-pass decoding on each endpoint to finalize segment text.

The goal is a “SuperWhisper-like” experience: immediate feedback while speaking, fast segment finalization when pausing, and stable UI updates.

This document is written from reading the code, and it references the current implementation for grounding. The code remains the source of truth.

## 1. Goals and Non-Goals

### 1.1 Goals

- Provide low-latency live dictation text while the user is speaking.
- Improve live text quality beyond sherpa streaming by running whisper in a rolling window in parallel.
- Preserve fast finalization semantics: after each endpoint, run whisper second pass and replace the provisional segment text.
- Avoid UI duplication bugs (for example, repeated hallucinated short phrases during silence).
- Keep UI changes minimal by preserving the existing event contract (`stt/partial`, `stt/final`) and the revision-based ordering model.
- Make all critical performance and quality knobs easy to tune (environment variables) so we can iterate quickly.

### 1.2 Non-Goals

- Do not introduce a second whisper context instance. A single whisper model should serve both sliding-window and second-pass work.
- Do not require modifications to `research/stt_compare` for the application pipeline. That tool is a reference harness.
- Do not change the UI state machine or keybindings unless a correctness issue requires it.
- Do not implement two-pass using a non-whisper offline model. The second pass remains whisper.

## 2. Glossary

- **PTT**: Push-to-talk. In AiR, holding Space starts dictation and releasing Space triggers finalization.
- **Partial**: A non-final transcript update emitted while audio is still incoming.
- **Endpoint**: A detected speech boundary (typically trailing silence). Endpointing splits audio into segments.
- **Segment**: A contiguous audio region between endpoints. Segments are indexed (`segment_id`) and stored as an ordered list.
- **Two-pass**: A pipeline that uses one model for partials and another model for finalization per segment. In AiR: sherpa for partials + whisper for final segment text.
- **Sliding-window**: Repeatedly decoding the last N seconds of audio (the “window”) every M milliseconds (the “step”) to refine a live transcript.

## 3. Component Inventory (End-to-End)

### 3.0 Technology Stack and Key Libraries

- Backend: Rust (edition 2024), Tauri v2, and Tokio.
- Frontend: React + TypeScript + Vite, using the Tauri events/commands bridge.
- Audio capture (macOS only): CoreAudio Voice Processing I/O (AudioUnit) producing mono float32 samples.
- Streaming STT: sherpa-onnx C API (`libsherpa-onnx-c-api.dylib`) with ONNX Runtime (`libonnxruntime.dylib`), loaded at runtime via `libloading`.
- Whisper: `whisper-rs` (whisper.cpp); macOS builds can use Metal for GPU acceleration.
- Data interchange: `serde` and `serde_json` for JSON parsing and event payloads.
- macOS setup: `script/setup_macos.sh` orchestrates `cmake` builds and model downloads for local development.

### 3.1 Frontend (UI)

- `ui/src/App.tsx`
  - Captures PTT key events:
    - Space down: `invoke("session_dispatch", { action: { type: "start_new" } })`.
    - Space up: `invoke("session_dispatch", { action: { type: "enter" } })`.
  - Subscribes to backend events:
    - `session/state`
    - `stt/partial`
    - `stt/final`
    - `llm/rewrite`
    - `error`
  - Uses `revision` monotonicity for `stt/partial` to ignore stale updates.
  - Computes `sttDelta` by prefix comparison:
    - If `nextText.startsWith(prevText)`, the delta is the appended suffix.
    - Otherwise the delta is empty (used by UI animation and perceived stability).

### 3.2 Backend (Tauri App)

- Session state machine:
  - `src-tauri/src/session.rs`
    - `SessionManager::dispatch(...)`
    - `SessionManager::run_sherpa_listening(...)` (current two-pass pipeline, and the insertion point for sliding-window work).
    - `SessionManager::set_stt_live_text(...)`
    - `SessionManager::commit_stt_segment_at(...)`
    - `SessionManager::replace_stt_segment_at(...)`
  - States (`SessionState`) include: `Hidden`, `Showing`, `Listening`, `Finalizing`, `Rewriting`, `RewriteReady`, `Injecting`, `Error`.

- Event contract:
  - `src-tauri/src/events.rs`
    - `stt/partial` payload includes: `session_id`, `revision`, `text`, `strategy`.
    - `stt/final` payload includes: `session_id`, `text`.
    - `strategy` currently supports `vad_chunk` and `sliding_window`. The UI currently treats them the same.

- Microphone capture:
  - `src-tauri/src/audio/mic_stream.rs`
    - Captures the default microphone using the Voice Processing I/O audio unit on macOS.
    - Requests 48 kHz float32 mono when available.
    - Produces chunks via a bounded Tokio channel to avoid backpressure in the audio callback.
    - Also records the full session audio (`MicRecording`) for later finalization.

- STT engines (wrappers):
  - sherpa:
    - `crates/sherpa-onnx` and `crates/sherpa-onnx-sys` provide dynamic loading + typed calls into the sherpa-onnx C API.
    - `src-tauri/src/stt.rs` provides `SherpaOnnxRecognizerManager` and config resolution.
  - whisper:
    - `src-tauri/src/stt/whisper.rs` provides:
      - `WhisperContextManager` (cached model load).
      - `WhisperConfig` (model path, language, threads, GPU override).
      - `transcribe(...)` and `resample_linear_to_16k(...)`.

### 3.3 Native Dependencies and Setup

- `script/setup_macos.sh` builds a minimal sherpa-onnx C API installation into:
  - `third_party/sherpa-onnx-prefix/`
- Runtime requires:
  - `third_party/sherpa-onnx-prefix/lib/libsherpa-onnx-c-api.dylib`
  - `third_party/sherpa-onnx-prefix/lib/libonnxruntime.dylib`
- Default models live under `model/`:
  - Streaming sherpa model: `model/sherpa-onnx-streaming-zipformer-en-2023-06-21/`.
  - Whisper GGML model: `model/whisper/ggml-large-v3-turbo-q8_0.bin`.

## 4. Current Two-pass Runtime Behavior (Baseline)

This section summarizes what the backend already does today, because the sliding-window design must integrate without breaking it.

### 4.1 Baseline Pipeline Overview

In `SessionManager::run_sherpa_listening(...)`:

1. Open microphone capture and record full-session audio.
2. Stream microphone chunks into sherpa:
   - `stream.accept_waveform(actual_sample_rate, samples)`.
   - `recognizer.decode(stream)` for incremental decoding.
   - Emit `stt/partial` when the sherpa transcript changes.
3. When sherpa reports `stream.is_endpoint()`:
   - Commit provisional segment text (from sherpa) into `stt_segments`.
   - Send the segment’s audio to a whisper worker thread for second pass.
   - When whisper completes, replace that segment text (in-place) and emit another `stt/partial`.

### 4.2 Baseline Segment Management

- Segments are identified by a monotonic `segment_id` starting at 1.
- Segments are stored in `SessionInner.stt_segments: Vec<String>`.
- A segment’s provisional text is written by `commit_stt_segment_at(...)`.
- A segment’s final text is written by `replace_stt_segment_at(...)`.
- The displayed text is assembled by `build_stt_text(stt_segments, stt_live_text)`.

### 4.3 Baseline Mitigations for Silence Bugs

To avoid repeated insertion during silence:

- Endpoints with an empty transcript are ignored (no segment is created).
- Whisper second-pass decoding is skipped for near-silent audio by a mean-absolute amplitude threshold.

This mitigation is critical and must remain compatible with the sliding-window pipeline.

## 5. Proposed Update: Parallel Sliding-window Whisper for Live Refinement

### 5.1 Summary

We add a “live refinement” loop that runs whisper on a rolling window in parallel with sherpa streaming:

- sherpa remains the lowest-latency signal and drives endpointing.
- whisper-window provides higher-quality live text that updates while speaking (every `step_ms`).
- whisper second pass remains the final authority for each endpoint segment.

### 5.2 Design Constraints

- Use a single whisper model context instance for the whole session.
- Avoid two concurrent whisper decodes at the same time (for throughput predictability and simpler resource management).
- Prefer correctness and UI stability over maximum update rate.
- Ensure that endpoint finalization has priority over whisper-window updates.

## 6. Audio Processing Logic (Mic → Buffers → Engines)

This section defines the audio representations and transformations used by each stage.

### 6.1 Microphone Input

- Device selection: default input device (system default microphone on macOS).
- Configuration:
  - Prefer 48 kHz if supported, else fallback to the OS default input config.
  - Use the device-provided sample format; convert to `f32`.
- Channel handling:
  - Mono input is passed through.
  - Multi-channel input is downmixed by averaging channels per frame.
- Normalization:
  - Each output sample is clamped to `[-1.0, 1.0]`.
- Output:
  - `MicStream` yields `Vec<f32>` chunks of mono samples at the chosen sample rate.
  - `MicRecording` captures the full session audio for finalization and debugging.

### 6.2 Chunking for sherpa Streaming

sherpa decoding is driven by fixed-size “read slices” to stabilize decoding behavior and endpointing.

Reference behavior in `research/stt_compare`:

- Default chunk size tuned to ~170 ms to reduce “stitching errors” observed with 100 ms chunks on `samples_jfk.wav`.

Proposed behavior in the app:

- Expose `AIR_SHERPA_ONNX_CHUNK_MS` (default `170`).
- Compute `samples_per_read = sample_rate_hz * chunk_ms / 1000`.
- Feed sherpa exactly `samples_per_read` samples per decode iteration.

Rationale:

- Smaller chunks reduce latency but can cause more hypothesis churn and boundary artifacts.
- Slightly larger chunks can improve stability and the quality of endpoint segmentation, which directly improves the two-pass stitching.

### 6.3 Resampling Strategy

- sherpa:
  - Is fed 16 kHz PCM via `accept_waveform(16000, samples_16k)`.
  - The pipeline resamples from the microphone sample rate to 16 kHz using `resample_linear_to_16k(...)`.

- whisper:
  - Is fed 16 kHz PCM.
  - The pipeline uses the same `resample_linear_to_16k(...)` path for both sliding-window and second-pass decode.

Important:

- Whisper-window and whisper second pass should share the same resampling path to keep behavior consistent.

### 6.4 Buffers (What We Store While Listening)

We maintain the following buffers during a session:

1. **Segment buffer (native sample rate)**:
   - Purpose: Provide exact audio for whisper second pass at endpoint.
   - Contents: All samples since the last endpoint (plus optional tail padding).
   - Owned by the sherpa streaming thread.

2. **Rolling window buffer (16 kHz)**:
   - Purpose: Provide audio for whisper sliding-window decoding.
   - Contents: The last `window_ms` of audio (plus optional context tail).
   - Implementation recommendation:
     - Use `VecDeque<f32>` as a ring buffer to avoid O(n) drains.
     - Keep capacity equal to `window_ms + context_ms` at 16 kHz.

3. **Optional committed tail buffer (16 kHz)**:
   - Purpose: Provide a small amount of context from committed audio to reduce whisper instability at the beginning of a new segment.
   - Default recommendation: `context_ms = 1000` (tunable).
   - This context must be accounted for during text de-duplication (Section 8).

### 6.5 Silence Gating

To avoid hallucinations during silence:

- For any whisper decode (window or second pass), compute mean absolute amplitude:
  - `mean_abs = sum(|sample|) / n`.
- If `mean_abs < threshold`, skip the decode.
- Threshold recommendation:
  - Default `0.001` (matches the current second-pass gate).
  - Expose `AIR_WHISPER_MIN_MEAN_ABS` (optional).

## 7. Concurrency Model (Threads, Tasks, and Backpressure)

### 7.1 Why a Single Whisper Worker

Even if the whisper library is thread-safe, running multiple concurrent decodes tends to create:

- Unpredictable latency spikes (contention on CPU/GPU).
- Backlogged window ticks (stale UI updates).
- Harder-to-debug timing and cancellation behaviors.

We prefer a single worker that serializes all whisper work and prioritizes segment finalization.

### 7.2 Task Roles

The listening session uses these concurrent roles:

1. **Mic reader (async task)**:
   - Pulls chunks from `MicStream`.
   - Sends chunks into an audio queue (`audio_tx`).
   - Never runs STT work.

2. **Streaming driver (blocking task)**:
   - Reads from `audio_rx`.
   - Chunk-slices into fixed `samples_per_read`.
   - Feeds sherpa (`accept_waveform` + `decode`).
   - Maintains segment buffers and rolling 16 kHz window buffers.
   - Produces:
     - sherpa partial updates.
     - endpoint segment boundary notifications.
     - whisper-window job submissions.
     - whisper second-pass job submissions.

3. **Whisper worker (blocking task)**:
   - Receives jobs and runs whisper decoding.
   - Emits results back to the async session loop via `update_tx`.

4. **Async session loop (async task)**:
   - Receives `AsrUpdate` messages and updates `SessionInner`:
     - `set_stt_live_text(...)`
     - `commit_stt_segment_at(...)`
     - `replace_stt_segment_at(...)`

### 7.3 Priority and Backpressure Requirements

We must prevent window jobs from delaying endpoint finalization:

- Endpoint second-pass jobs are **high priority**.
- Window jobs are **low priority** and may be dropped if the worker is busy.

Recommended queue design:

- A high-priority unbounded `std::sync::mpsc::Receiver<SecondPassJob>`.
- A low-priority bounded `std::sync::mpsc::SyncSender<WindowJob>` with capacity 1–2.
  - The producer uses `try_send`.
  - If full, the tick is dropped (the next tick will carry fresher audio).

Whisper worker loop pseudo-code:

1. Drain any pending high-priority jobs first (`try_recv` loop).
2. If none, block on low-priority `recv_timeout(...)`.
3. Repeat until cancellation or channel close.

### 7.4 Cancellation Semantics

- The session owns a cancellation token used to stop the mic reader and end the listening loop.
- On finalization, we allow outstanding whisper jobs to complete so that `stt/final` reflects the best available transcript.
- Window jobs should be discardable on cancellation without additional cleanup.

## 8. Transcript Assembly and UI Stability

This is the most user-facing part of the design. The UI expects a stable, mostly append-only experience.

### 8.1 Text Sources and Precedence

At any point, the “display text” is composed from:

1. **Committed segments** (`stt_segments`):
   - Provisional sherpa text at endpoint, later replaced by whisper second-pass.
2. **Live sherpa text** (`stt_live_text`):
   - Lowest-latency text, can revise frequently.
3. **Live whisper-window text** (new):
   - Higher-quality live text, updated every `step_ms`.

Precedence rules:

- While whisper-window is not available (startup, busy worker, dropped ticks), display sherpa live text.
- When whisper-window is available, it becomes the preferred live text and should replace sherpa live text in the UI.
- After an endpoint:
  - The segment is committed (provisionally) immediately.
  - Live text resets to empty for the next segment.
  - whisper-window continues for the new live segment (and may use a small context tail).

### 8.2 Preventing Duplicate Text When Using a Global Window

If whisper-window decodes a window that includes already committed audio, the output will contain words that are already in `stt_segments`.
If we naively append whisper-window output, the UI will show repeated phrases.

Therefore, whisper-window output must be converted into a “live suffix” that excludes the committed prefix.

Recommended approach (token overlap removal):

1. Compute `committed_text = join(stt_segments)`.
2. Compute `window_text = whisper_window_output`.
3. Tokenize both into words:
   - Split on whitespace.
   - Normalize each token for matching (lowercase, trim punctuation).
4. Find the best overlap:
   - Let `A` be the last `K` tokens of `committed_text` (for example, `K=30`).
   - Find the longest contiguous match of a suffix of `A` inside `window_text` tokens.
   - Accept matches down to a minimum length (for example, `min=4`) to avoid accidental overlaps.
5. If a match is found at `window[i..j]`, define:
   - `live_tokens = window[j..]`.
   - `live_text = join(live_tokens)`.
6. If no match is found:
   - Fallback to a safer mode:
     - Either use sherpa live text for this tick, or
     - Use the raw whisper-window text but do not append it to the committed prefix (replace-only display for that tick).

Rationale:

- Exact token overlap is fast, deterministic, and easy to debug.
- It is robust to small punctuation differences.
- It may fail on “your/you're”-type differences; that is why we need the fallback.

Optional improvement (fuzzy token equality):

- Treat tokens as equal if:
  - They match after removing apostrophes, or
  - Their edit distance is <= 1 for short tokens.

### 8.3 Stability Strategy (Reducing Flicker)

Even with overlap removal, whisper-window output can revise earlier words in the live suffix.
The UI’s `sttDelta` animation works best when updates are append-only.

Recommended stability mechanism for the live suffix:

- Maintain a per-session `live_committed_prefix` and `live_unstable_suffix`.
- On each new whisper-window result:
  1. Compute `candidate_live = overlap_removed_live_text`.
  2. Compare `candidate_live` to the previous `candidate_live_prev`.
  3. Commit words into `live_committed_prefix` only when they have remained stable for `N` consecutive ticks.
  4. Keep the rest in `live_unstable_suffix`.
- Display:
  - `display_text = committed_segments_text + live_committed_prefix + live_unstable_suffix`.

Default recommendations:

- `N = 2` or `3` (tunable).
- Emit updates at most once per tick, and only when the display text changes.

This yields a UI experience where:

- Text mostly grows forward.
- Corrections happen within the unstable tail, not across the whole transcript.

### 8.4 Segment Stitching Between sherpa and whisper

Observed tuning from the A/B harness:

- Increasing sherpa chunk size from 100 ms to ~170 ms reduced stitching errors in two-pass output on `samples_jfk.wav`.

To preserve this in the app:

- The segment boundaries are still defined by sherpa endpointing.
- The second-pass whisper result replaces only the segment text at `segment_id`.
- The sliding-window live suffix should not “commit” into `stt_segments` directly; it is a live view.

## 9. UI Interaction Contract (What the UI Sees)

### 9.1 Events Emitted by the Backend

- `session/state`
  - Notifies UI about session lifecycle (showing, listening, finalizing, etc.).
- `stt/partial`
  - `text` is the full current transcript (committed + live).
  - `revision` is strictly increasing per session.
  - `strategy` indicates the source of the update:
    - `vad_chunk` for sherpa-driven updates (and segment commit/replace).
    - `sliding_window` for whisper-window-driven updates.
- `stt/final`
  - Sent when the session finalizes. It should reflect:
    - All committed segments.
    - Any segment replacements produced by whisper second pass that completed before finalization.

### 9.2 UI Behavior (Current)

From `ui/src/App.tsx`:

- `stt/partial` is applied only if `payload.revision > lastRevision`.
- The UI derives an “append delta” only when the new text is a strict prefix extension of the old text.
- The UI does not currently treat strategies differently.

Implications for the backend:

- Use `revision` ordering to handle out-of-order updates from multiple producers.
- Prefer emitting mostly append-only updates for the live suffix (Section 8.3), but accept that:
  - Endpoint replacements will rewrite older text and reset the UI delta.
  - This is expected and is a trade-off for accuracy.

### 9.3 UI Commands That Affect STT

- PTT keydown triggers `session_dispatch start_new`.
- PTT keyup triggers `session_dispatch enter` (finalize).
- `escape` cancels the session and hides the overlay.

The sliding-window design does not require new commands.

## 10. Configuration and Tuning Surface

This section lists the configuration knobs needed to reproduce `research/stt_compare` tuning in the main app.

### 10.1 sherpa Tuning

Existing:

- `AIR_SHERPA_ONNX_PROVIDER`
- `AIR_SHERPA_ONNX_DECODING_METHOD`
- `AIR_SHERPA_ONNX_MAX_ACTIVE_PATHS`
- Endpoint rules:
  - `AIR_SHERPA_ONNX_RULE1_MIN_TRAILING_SILENCE`
  - `AIR_SHERPA_ONNX_RULE2_MIN_TRAILING_SILENCE`
  - `AIR_SHERPA_ONNX_RULE3_MIN_UTTERANCE_LENGTH`

Proposed new:

- `AIR_SHERPA_ONNX_CHUNK_MS`
  - Default: `170`.
  - Effect: Controls fixed chunk size for sherpa `accept_waveform` and `decode` loop in the app.

### 10.2 whisper Model and Runtime

Existing:

- `AIR_WHISPER_MODEL_PATH`
- `AIR_WHISPER_LANGUAGE` (`en` by default, `auto` supported).
- `AIR_WHISPER_NUM_THREADS`
- `AIR_WHISPER_FORCE_GPU`

### 10.3 whisper Sliding-window (New)

Proposed new:

- `AIR_WHISPER_WINDOW_ENABLED`
  - Default: `true`.
- `AIR_WHISPER_WINDOW_MS`
  - Default: `8000`.
- `AIR_WHISPER_STEP_MS`
  - Default: `500` (based on A/B harness tuning).
- `AIR_WHISPER_TICK_EVERY`
  - Default: `1` for UI updates (emit each decode if changed).
  - Note: In `research/stt_compare`, this is a print-throttling knob, not a decode knob. In the app it should control emission throttling.
- `AIR_WHISPER_WINDOW_CONTEXT_MS`
  - Default: `1000` (optional; set `0` to disable).
- `AIR_WHISPER_MIN_MEAN_ABS`
  - Default: `0.001`.

### 10.4 whisper Decode Parameters (New)

To match `research/stt_compare` behavior:

- `AIR_WHISPER_BEST_OF`
  - Range: `1` to `8`.
  - Default: `5`.
  - Guard: reject values > 8 to avoid whisper.cpp internal errors observed in the harness.
- `AIR_WHISPER_BEAM_SIZE`
  - Optional. When set, use beam search instead of greedy.
- `AIR_WHISPER_BEAM_PATIENCE`
  - Optional beam search patience.

## 11. Observability and Debugging

Recommended debug outputs (behind env flags):

- Log selected microphone sample rate and channel config at session start.
- Log sherpa chunk size (`chunk_ms`) and endpoint rule values.
- Log window decode schedule (`window_ms`, `step_ms`) and whether ticks are being dropped.
- Log per-segment second-pass latency (“endpoint → whisper final”) to quantify UX.

All logs must be in clear English and must not include machine-specific absolute paths.

## 12. Implementation Plan (Phased)

This is an ordered plan to add sliding-window behavior without destabilizing two-pass finalization.

### Phase 1: Configuration Plumbing

- Add env-backed config for:
  - `AIR_SHERPA_ONNX_CHUNK_MS`
  - whisper-window settings and whisper decode settings.
- Ensure defaults match the tuned values from `research/stt_compare`.

Acceptance:

- Build passes `cargo make fmt`, `cargo make clippy`, `cargo make nextest`.

### Phase 2: Rolling 16 kHz Buffer and Window Job Scheduling

- In the streaming driver:
  - Maintain a 16 kHz rolling buffer.
  - Every `step_ms`, generate a window job (if enabled).
  - Drop ticks if a window job is already queued.

Acceptance:

- Window jobs are submitted at the configured cadence during mic input.

### Phase 3: Single Whisper Worker with Priority

- Add a window-job receiver to the whisper worker.
- Ensure:
  - Second-pass jobs are processed before window jobs.
  - Window jobs do not backlog.

Acceptance:

- Endpoint finalization latency does not regress compared to baseline.

### Phase 4: Overlap Removal and Stability Commit

- Implement overlap removal and a simple stability mechanism so UI text is mostly append-only.
- Emit `stt/partial` with `strategy=sliding_window` for window-driven updates.

Acceptance:

- No repeated phrases appear in the UI due to overlap.
- UI updates are stable and do not flicker aggressively.

### Phase 5: Tune and Document

- Validate on the same samples used by `research/stt_compare` and on microphone input.
- Update `spec/50_speech_to_text.md` with the new window configuration knobs.

Acceptance:

- Sliding-window partials are visibly more accurate than sherpa partials.
- Two-pass final segments remain correct and do not duplicate across boundaries.

## 13. Appendix: Code References (Current)

These are the primary code locations referenced by this spec:

- Session and STT orchestration:
  - `src-tauri/src/session.rs`
- Microphone capture:
  - `src-tauri/src/audio/mic_stream.rs`
- sherpa configuration and model selection:
  - `src-tauri/src/stt.rs`
- whisper model management and decoding:
  - `src-tauri/src/stt/whisper.rs`
- UI event handling and PTT:
  - `ui/src/App.tsx`
- Reference harness for parameter tuning:
  - `research/stt_compare/src/whisper_window.rs`
  - `research/stt_compare/src/whisper.rs`
  - `research/stt_compare/src/cli.rs`
