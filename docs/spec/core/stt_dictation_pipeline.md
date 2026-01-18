# STT Dictation Pipeline Spec (Canonical v2)

Purpose: Define the canonical dictation pipeline for InkFlow.

Audience: Engineers and LLMs reading the system specification.

Scope: End-to-end dictation behavior, including streaming partials, live refinement, and finalization.

This document is a single, comprehensive specification of InkFlow's dictation STT pipeline:

- **sherpa-onnx streaming** for low-latency partials and endpoint detection.
- **Whisper sliding-window decoding** in parallel during PTT to refine the live transcript while speaking.
- **Whisper second-pass decoding** on each endpoint (and on forced-finalize) to finalize segment text.

This spec is implementation-oriented and code-referenced. The code remains the source of truth, and this document must be updated when behavior changes.

---

## Goals and non-goals

### Goals

- Provide low-latency live dictation while the user is speaking.
- Improve live text quality via Whisper sliding-window refinement.
- Preserve fast finalization semantics with Whisper second pass per endpoint.
- Avoid UI duplication bugs (for example, repeated short phrases during silence).
- Keep the existing UI event contract stable (`stt/partial`, `stt/final`).
- Keep performance and quality knobs adjustable from the UI.
- Support English and Chinese input in the current implementation.

### Non-goals

- Do not introduce a second Whisper context instance.
- Do not change the UI state machine or keybindings unless required for correctness.
- Do not change the STT comparison harness to match app behavior.

## Glossary

- **PTT**: Push-to-talk. Holding Space starts dictation; releasing Space triggers finalization.
- **Partial**: A non-final transcript update emitted while audio is still incoming.
- **Endpoint**: A detected speech boundary (typically trailing silence).
- **Segment**: A contiguous audio region between endpoints.
- **Two-pass**: sherpa streaming for partials plus Whisper for endpoint finalization.
- **Sliding-window**: Repeatedly decoding the last N seconds of audio every M milliseconds to refine live text.

## 0. Implementation Status (Code vs Spec)

This document specifies the target architecture. Some parts already exist in code, and some parts are planned.

### Implemented today (baseline)

- Sherpa-onnx streaming partials and endpoint detection.
- Segment-based transcript model (`stt_segments` + `stt_live_text`) and revision-ordered `stt/partial` events.
- Whisper second pass on each endpoint that replaces the provisional segment text in-place.
- Silence mitigations:
  - Ignore empty endpoints.
  - Skip Whisper on near-silent audio.
- **Mode routing (current)**:
  - Only local stream + local second pass is implemented.
  - Optional local sliding-window refinement is enabled by settings.
  - All other mode combinations are TODO.

### Implemented/Required in v2 (must exist to claim v2 compliance)

- **Forced-finalize behavior** on `Enter`:
  - Finalize must not depend on a natural sherpa endpoint.
- **Window scheduling that never backlogs**:
  - Bounded queue + drop ticks.
- **Stale window result rejection**:
  - Window results must be dropped if computed on an older audio snapshot / generation.
- **Language-aware / timestamp-aware de-duplication**:
  - Robust for languages without whitespace (e.g., Chinese).
- **Live stability mechanism with rollback**:
  - UI should see mostly append-only changes, but must allow limited corrections.
- **UI-first Settings**:
  - All tunables adjustable from UI (env vars are dev-only, optional).
---

## 1. User Experience Contract

### 1.1 Core UX Goals

- The overlay appears, the user holds Space, and the transcript starts updating quickly.
- While speaking:
  - UI shows near-real-time partial text (low latency).
  - Text becomes increasingly accurate as more audio arrives (refinement).
- When the user pauses or releases Space:
  - The last spoken segment finalizes quickly and reads "clean" (punctuation, fewer homophone errors).
  - Final text remains stable for downstream steps (rewrite/inject).

### 1.2 Start/Stop Lifecycle (UI-driven)

The SwiftUI app controls dictation explicitly:

- Start: registers the FFI callback and begins microphone capture.
- Stop: stops microphone capture and unregisters the callback.

Backend is the single source of truth for transcript updates and timing.

Code anchor: `apps/macos/InkFlow/InkFlow/ContentView.swift`, `apps/macos/InkFlow/InkFlow/InkFlowViewModel.swift`, `crates/inkflow-ffi/src/lib.rs`.

### 1.3 Finalization Must Capture the Last Words (Forced Finalize)

**Spec requirement (v2):** On `Enter`, the system must finalize the last in-progress speech even if sherpa did not produce a natural endpoint.

On `Enter`:

1. Stop mic capture / stop accepting new audio.
2. Perform a **forced flush** of the streaming pipeline:
   - If sherpa exposes an input-finished / flush mechanism, use it.
   - Otherwise proceed using buffered audio only.
3. If there is buffered audio since the last committed segment:
   - If it passes silence gating **and** minimum duration requirements, commit a provisional segment and enqueue a second-pass Whisper job.
   - If it is near-silent, do not create a segment.
4. Wait for required second-pass job(s) (at least the final segment) and then emit `stt/final`.

This removes the "last words missing / last segment not cleaned up" failure mode.

### 1.4 Text Update Semantics (What the UI Expects)

UI receives JSON updates through the FFI callback. The SwiftUI app builds the displayed transcript from:

- `sherpa_partial` and `window_result` for live text.
- `segment_end` for committed segments.
- `second_pass` to replace a committed segment with Whisper output.

Implication:

- The Rust engine should emit updates that are mostly append-only in the live tail.
- The UI must tolerate rewrites when accuracy improves.

Code anchor: `apps/macos/InkFlow/InkFlow/InkFlowViewModel.swift`, `crates/inkflow-ffi/src/lib.rs`.

---

## 2. Technology Stack and Key Libraries

### 2.1 UI

- SwiftUI on macOS 26.
- Liquid Glass for primary surfaces and controls.

### 2.2 Backend

- Rust (edition 2024).
- `inkflow-core` for pipeline logic.
- `inkflow-ffi` for the C ABI callback bridge.
- Tokio (async orchestration, channels, task spawning).

### 2.3 Audio

- Microphone capture on macOS using AVAudioEngine.
- Produces mono `float32` buffers at the device sample rate.

### 2.4 STT Engines

- Sherpa-onnx (streaming Zipformer-Transducer) via C API.
- Whisper via `whisper-rs` (whisper.cpp).

### 2.5 Serialization and Contracts

- `serde` / `serde_json`.

---

## 3. System Components (End-to-End)

### 3.1 UI Component

`apps/macos/InkFlow/InkFlow`:

- Starts and stops dictation using explicit controls in SwiftUI.
- Registers the FFI callback and renders transcript updates.
- Dispatches all UI updates onto the main thread.
- Uses Liquid Glass for primary surfaces and controls on macOS 26.

### 3.2 Backend Engine

`crates/inkflow-core/src/engine.rs`:

- Owns the STT pipeline and exposes `InkFlowEngine` for FFI use.
- Receives audio buffers, manages sherpa streaming, and schedules Whisper work.
- Emits JSON update events via the FFI layer.
- Maintains segment and window state required for refinement and finalization.

### 3.3 Audio Capture

`apps/macos/InkFlow/InkFlow/AudioCapture.swift`:

- Captures microphone audio via AVAudioEngine.
- Requests mono float32 buffers at the device sample rate.

### 3.4 STT Wrappers and Configuration

- Sherpa: `crates/inkflow-core/src/stt/mod.rs`.
- Whisper: `crates/inkflow-core/src/stt/whisper.rs`.

### 3.5 Engine Manager + Settings (v2 required)

**Spec requirement (v2):** Replace env-variable configuration with a persisted Settings model adjustable from UI.

New component: `EngineManager` (or equivalent):

- Owns the currently loaded sherpa recognizer factory / model handles.
- Owns the WhisperContext (exactly one at a time).
- Applies new settings via:
  - "soft apply" (no reload) → affects subsequent decode jobs.
  - "engine reload" (recreate recognizer/context) → may take seconds but stays in-process.
  - "restart-required" only for low-level dynamic library path switching (developer-only).

---

## 4. Backend ↔ UI Contract (Events and Commands)

### 4.1 Backend → UI Events

- `session/state`
- `stt/partial`:
  - `session_id`, `revision`, `text`, `strategy`
  - `strategy`: `vad_chunk` | `sliding_window`
- `stt/final`
- `llm/rewrite`
- `error`
- Optional `engine/state` (recommended)

### 4.2 UI → Backend Commands

Existing:
- `session_dispatch(action)` (`start_new` | `enter` | `escape`)
- `overlay_set_height(height, animate)`

Recommended v2:
- `settings_get()`
- `settings_update(patch)` (persist only)
- `engine_apply_settings(patch)` (persist + apply; returns apply-level)

---

## 5. Audio Pipeline Specification

### 5.1 Sample Representation

All audio samples used by the STT pipeline are:

- Mono `f32`, clamped to `[-1.0, 1.0]`.

### 5.2 Microphone Sample Rate

Mic sample rate is device-defined; prefer 48 kHz.

### 5.3 Chunking for Engine Feeding

Two chunking concepts:

1. Mic chunks: from the microphone capture callback.
2. Decode slices: fixed-size slices fed into sherpa.

Spec requirement:
- Sherpa must be fed fixed-size decode slices.
- `chunk_ms` must be configurable.

Recommended default:
- `sherpa_chunk_ms = 170`.

### 5.4 Resampling

- Sherpa: is fed 16 kHz PCM (the pipeline resamples microphone audio to 16 kHz before calling `accept_waveform(...)`).
- Whisper: is fed 16 kHz PCM (the pipeline uses the same linear resampler path).

### 5.5 Buffers Kept During Listening

1. Segment buffer (native sample rate)
2. Window ring buffer (16 kHz)
3. Optional committed context tail (16 kHz)

### 5.6 Silence Gating

Compute:
- `mean_abs = sum(|x|) / n`

Skip Whisper decode when:
- `mean_abs < min_mean_abs`

Applies to:
- Sliding-window jobs.
- Second-pass jobs (including forced-finalize)
---

## 6. STT Pipeline Behavior (Three Signals, One Transcript)

### 6.1 Segment Model

- `stt_segments`: committed segments (one per endpoint)
- `stt_live_text`: live tail (in-progress segment)

Second-pass replaces a segment in-place using `segment_id`.

### 6.2 sherpa Streaming Responsibilities

Sherpa provides:

- Low-latency partials.
- Endpoint detection (`is_endpoint()`)
On endpoint:

- If sherpa text is non-empty: commit provisional segment immediately.
- If sherpa text is empty:
  - Do **not** commit only when audio is near-silent or too short.
  - If audio is non-silent and long enough, still create a provisional segment (may be empty) and run second-pass.

Then:
- Clear `stt_live_text`.
- Increment `window_generation`.
- Reset live stability state.

### 6.3 Whisper Second-pass Responsibilities

On each endpoint + forced-finalize:

- Decode endpoint audio.
- Replace segment text.
- Emit `stt/partial` (revision++)

### 6.4 Whisper Sliding-window Responsibilities

While speaking:

- Every `step_ms`, decode last `window_ms` (plus optional `context_ms`)
- Merge into live tail without duplicating committed text.
- Emit `stt/partial` with `strategy=sliding_window`.
Sliding-window must never mutate committed segments.

---

## 7. Concurrency and Scheduling (Single Whisper Instance, Priority)

### 7.1 Single WhisperContext

Exactly one Whisper model instance (WhisperContext) at a time.

### 7.2 Worker Roles

- Mic reader (async)
- Sherpa driver + scheduler (blocking)
- Whisper worker (blocking)
- Session update loop (async)

### 7.3 Priority Policy (Non-preemptive Reality Acknowledged)

- High-priority: second-pass jobs
- Low-priority: window jobs

Rules:

1. second-pass jobs are never queued behind window jobs (queue discipline).
2. window jobs never backlog:
   - Bounded queue capacity 1–2.   - `try_send`; drop when full
3. non-preemptive note:
   - If a window decode has started, second-pass must wait for it to finish.   - Keep window decode cheap.

### 7.4 Separate Decode Profiles (strongly recommended)

- Window profile: fast (e.g., greedy, best_of=1)
- Second-pass profile: accurate (best_of=5, cap ≤ 8, optional beam)

### 7.5 Budget-aware Window Throttling (recommended)

If window decode time approaches/exceeds `step_ms`, reduce window frequency or pause window decoding temporarily.

### 7.6 Cancellation and Finalization

- `escape`: stop capture, drop window, do not wait for second-pass
- `enter`: stop capture, forced-finalize if needed, wait for required second-pass, then emit `stt/final`

---

## 8. Transcript Merge Rules (Preventing Duplicates and UI Flicker)

### 8.1 Precedence Rules

- Live tail: prefer whisper-window; fallback to sherpa
- Committed segments: final always from second-pass

### 8.2 Stale Window Result Rejection (v2 required)

Each window job includes:
- `job_id`
- `generation`
- `window_end_16k_samples`

Apply only if:
- `generation == current_generation`
- `job_id` is the latest (or >= last_applied)
- End sample not older than current expected end.
Otherwise drop result.

### 8.3 Timestamp-aware Merge (preferred)

Decode window with timestamps.
Compute committed boundary in 16k samples: `committed_end_16k_samples`.

Select whisper segments that end after:
- `cut = committed_end_16k_samples - context_len_16k`

Concatenate selected text to form `window_tail_text`.

Boundary de-dup with short token overlap (8.4).

Fallback if timestamps unavailable:
- Language-aware token overlap across committed text and window text.

### 8.4 Language-aware Token Overlap (Boundary De-dup)

Tokenization:
- Whitespace languages: word tokens (lowercase, strip edge punctuation)
- CJK/no-whitespace: grapheme or char tokens

Use K:
- Words: 30.
- Chars/graphemes: 80–120.
Remove duplicated prefix in `window_tail_text`.

### 8.5 Live Stability with Rollback (v2 required)

Maintain:
- Stable prefix.
- Unstable tail.
Promotion:
- Tokens become stable only after unchanged for N consecutive accepted ticks.
Rollback:
- If LCP shrinks beyond threshold, rollback stable prefix to LCP boundary and restart counting.

### 8.6 Endpoint Interaction

On endpoint (natural or forced):
- Commit segment.
- Clear live tail.
- Increment generation.
- Reset stability state.
- Update committed boundary time in 16k.

### 8.7 Empty/Whitespace Update Rules

- Do not emit `stt/partial` if the only change is empty/whitespace-only live tail.
- Do not create segments for silent endpoints.
- Do not run whisper on near-silent audio.
---

## 9. Settings and Tuning Knobs (UI-first)

### 9.1 Settings Source of Truth

Release builds:
- Settings come from UI + persisted file.
- Env vars are not used.

Dev builds (optional):
- Env vars may override for developer convenience.

### 9.2 Settings Model (summary)

- Sherpa: model_dir, provider, threads, decoding method, endpoint rules, chunk_ms, int8 prefs.
- Whisper: model_path, language, threads, force_gpu.
- Whisper profiles:
- Window profile (fast)  - Second-pass profile (accurate)
- Sliding-window: enabled, window_ms, step_ms, context_ms, min_mean_abs, emit_every.
- Merge/stability: N, rollback_threshold, overlap_K (word/char)

### 9.3 Apply Levels (no restart for normal users)

- `soft_applied`: no reload needed
- `reloaded`: recreate engine in-process
- `restart_required`: only for dynamic library path switching (developer-only)

Normal UI must not require restart.

### 9.4 Validation

- `best_of`: 1..=8
- Window_ms >= step_ms.
- Chunk_ms within safe bounds.
- Reject invalid settings atomically (no partial apply)
---

## 10. Observability and Debugging

Env-gated or settings-gated logs (no absolute paths):

- Session start: mic SR, sherpa config, window config, decode profiles.
- Endpoint: segment id, duration, second-pass latency.
- Window tick: dropped tick, stale drop, decode time vs budget.
- Engine apply: apply level, reload duration.
---

## 11. Reference Code Locations

- UI: `apps/macos/InkFlow/InkFlow/ContentView.swift`
- UI state and callbacks: `apps/macos/InkFlow/InkFlow/InkFlowViewModel.swift`
- FFI bridge: `crates/inkflow-ffi/src/lib.rs`
- Mic capture: `apps/macos/InkFlow/InkFlow/AudioCapture.swift`
- Sherpa: `crates/inkflow-core/src/stt/mod.rs`.
- Whisper: `crates/inkflow-core/src/stt/whisper.rs`.
- Harness: `research/stt_compare/...`
- Settings: `crates/inkflow-core/src/settings.rs`
- Engine: `crates/inkflow-core/src/engine.rs`
- Implementation notes: `docs/guide/development/stt_dictation_pipeline_impl_notes.md`

---

## 12. Status and Maintenance

- This is the canonical spec for "parallel sherpa + whisper-window + whisper second pass" (v2).
- If implementation diverges, update this document and related specs.

---

# Appendix A: Minimal "Must Pass" Behavioral Tests (v2)

1. Forced finalize correctness:
   - Speak then immediately release Space; `stt/final` must include last words.
2. No duplicate committed text from window:
   - Two sentences with a pause; window must not re-emit sentence 1 in tail.
3. Chinese stability:
   - Chinese dictation without spaces; no duplicated clauses; minimal flicker.
4. Stale window rejection:
   - Artificially slow window decode; late results must not overwrite newer tail.
5. Priority under load:
   - Second-pass latency remains bounded; window must drop ticks and stay cheap.

---

# Appendix B: Engine Apply / Reload State Machine + Rust-like Pseudocode (v2)

This appendix defines how to apply UI-adjusted settings without requiring an app restart (except dev-only dynamic library changes).

## B.1 Key requirements

- Applying settings is **atomic** (no partial apply).
- Applying settings must not break in-flight sessions:
  - Recommended: block engine reload while listening; require user to stop dictation first.
- Soft changes apply immediately to subsequent decode jobs (no reload).
- Reload changes re-create engine in-process and emit `engine/state` events.
- Restart-required changes are developer-only (e.g., changing C API dylib path).

## B.2 Apply Level Classification

Define 3 apply levels:

- `SoftApplied`: no engine objects need to be dropped/recreated.
- `Reloaded`: requires in-process engine reload (drop + recreate sherpa/whisper).
- `RestartRequired`: requires app restart to be safe (developer-only).

### B.2.1 Typical classification

**SoftApplied (no reload)**
- Whisper decode params (window/second-pass):
  - Best_of, beam_size, beam_patience, language.
- Sliding-window knobs:
- Enabled, window_ms, step_ms, context_ms, min_mean_abs, emit_every.
- Merge/stability knobs:
- Overlap_K, N, rollback_threshold.
**Reloaded (in-process re-init)**
- Whisper model_path
- Whisper force_gpu (if implementation requires recreating context)
- Whisper threads (if fixed at init)
- Sherpa: provider, threads, decoding method, model_dir, int8 prefs
- Sherpa endpoint rules, chunk_ms (if not per-stream adjustable)

**RestartRequired (developer-only)**
- Sherpa C API dylib path switching
- ONNX runtime dylib path switching / incompatible runtime changes

## B.3 Engine State Machine

State enum:

- `Ready`
- `Reloading { started_at, reason }`
- `Error { message }`

Transitions:

- `Ready -> Reloading`: on `engine_apply_settings` when apply level is Reloaded
- `Reloading -> Ready`: on successful re-init
- `Reloading -> Error`: on failure
- `Error -> Reloading`: on retry apply (optional)
- `Error -> Ready`: on restore last-known-good settings (optional)

UI behavior:

- When `Reloading`, disable PTT and show "Loading speech engine…".
- On `Error`, show error and allow retry.

## B.4 Engine Generation (Safety against stale results)

Maintain a global `engine_generation: AtomicU64`.

- Increment generation whenever:
  - Engine is reloaded.
- A session is force-canceled due to engine changes.
- Each whisper job carries:
  - `engine_generation` snapshot
  - `window_generation` snapshot (session-level)
  - `job_id`

When results arrive:
- If generation mismatch, drop result silently.

This prevents old workers/results from contaminating current state.

## B.5 Suggested structure

- `SettingsStore`: persists settings to disk.
- `EngineManager`: owns engine objects and applies settings.
- `SessionManager`: runs dictation sessions and requests decoding.
- `WhisperWorker`: one worker that executes jobs, high priority first.

## B.6 Rust-like pseudocode

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
  pub sherpa: SherpaSettings,
  pub whisper: WhisperSettings,
  pub window: WindowSettings,
  pub merge: MergeSettings,
  pub profiles: WhisperProfiles, // window_profile + second_pass_profile
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ApplyLevel {
  SoftApplied,
  Reloaded,
  RestartRequired,
}

pub fn classify_apply_level(old: &Settings, new: &Settings) -> ApplyLevel {
  if old.sherpa.dylib_path != new.sherpa.dylib_path {
    return ApplyLevel::RestartRequired;
  }

  let reload_fields_changed =
    old.whisper.model_path != new.whisper.model_path ||
    old.whisper.force_gpu != new.whisper.force_gpu ||
    old.sherpa.model_dir != new.sherpa.model_dir ||
    old.sherpa.provider != new.sherpa.provider ||
    old.sherpa.num_threads != new.sherpa.num_threads ||
    old.sherpa.decoding_method != new.sherpa.decoding_method ||
    old.sherpa.max_active_paths != new.sherpa.max_active_paths ||
    old.sherpa.prefer_int8 != new.sherpa.prefer_int8 ||
    old.sherpa.use_int8_decoder != new.sherpa.use_int8_decoder ||
    old.sherpa.endpoint_rules != new.sherpa.endpoint_rules ||
    old.sherpa.chunk_ms != new.sherpa.chunk_ms;

  if reload_fields_changed {
    ApplyLevel::Reloaded
  } else {
    ApplyLevel::SoftApplied
  }
}

pub struct EngineManager {
  state: tokio::sync::RwLock<EngineState>,
  settings: tokio::sync::RwLock<Settings>,

  // Engine objects:
  whisper_ctx: tokio::sync::RwLock<Option<WhisperContext>>,
  sherpa_factory: tokio::sync::RwLock<Option<SherpaRecognizerFactory>>,

  engine_generation: std::sync::atomic::AtomicU64,
}

impl EngineManager {
  pub async fn settings_get(&self) -> Settings {
    self.settings.read().await.clone()
  }

  pub async fn settings_set_persist_only(&self, new: Settings) -> anyhow::Result<()> {
    validate_settings(&new)?;
    SettingsStore::save(&new).await?;
    *self.settings.write().await = new;
    Ok(())
  }

  pub async fn engine_apply_settings(&self, new: Settings, session_mgr: &SessionManager)
    -> anyhow::Result<ApplyLevel>
  {
    validate_settings(&new)?;

    let old = self.settings.read().await.clone();
    let level = classify_apply_level(&old, &new);

    // Always persist first (atomic write recommended).
    SettingsStore::save(&new).await?;

    match level {
      ApplyLevel::SoftApplied => {
        *self.settings.write().await = new;
        // No engine object changes required.
        return Ok(ApplyLevel::SoftApplied);
      }

      ApplyLevel::Reloaded => {
        // Safety: do not reload while actively listening.
        if session_mgr.is_listening().await {
          anyhow::bail!("Stop dictation before applying engine reload changes.");
        }

        // Update settings early so new init uses them.
        *self.settings.write().await = new.clone();

        // Enter Reloading
        self.set_engine_state(EngineState::Reloading { reason: "settings changed" }).await;

        // Bump generation so stale results are dropped.
        self.engine_generation.fetch_add(1, Ordering::SeqCst);

        // Drop and recreate engine objects (in-process).
        let res = self.reload_engine_objects(&new).await;

        match res {
          Ok(()) => {
            self.set_engine_state(EngineState::Ready).await;
            Ok(ApplyLevel::Reloaded)
          }
          Err(e) => {
            self.set_engine_state(EngineState::Error { message: format!("{e:#}") }).await;
            Err(e)
          }
        }
      }

      ApplyLevel::RestartRequired => {
        // Developer-only: do not expose in normal UI.
        Ok(ApplyLevel::RestartRequired)
      }
    }
  }

  async fn reload_engine_objects(&self, s: &Settings) -> anyhow::Result<()> {
    // 1) Drop whisper context
    {
      let mut guard = self.whisper_ctx.write().await;
      *guard = None;
    }

    // 2) Drop sherpa factory
    {
      let mut guard = self.sherpa_factory.write().await;
      *guard = None;
    }

    // 3) Recreate whisper context (loads model file)
    let whisper = WhisperContext::load(&s.whisper.model_path, s.whisper.force_gpu, s.whisper.num_threads)?;
    *self.whisper_ctx.write().await = Some(whisper);

    // 4) Recreate sherpa factory/recognizer config
    let sherpa = SherpaRecognizerFactory::new(&s.sherpa)?;
    *self.sherpa_factory.write().await = Some(sherpa);

    Ok(())
  }

  async fn set_engine_state(&self, st: EngineState) {
    *self.state.write().await = st.clone();
    emit_engine_state_event(st).await;
  }

  pub fn current_engine_generation(&self) -> u64 {
    self.engine_generation.load(Ordering::SeqCst)
  }
}
```

### B.6.1 Notes on user experience

- For `Reloaded`, UI should show a small spinner "Loading speech engine…" and disable dictation for that duration.
- Most users should never see `RestartRequired`. Keep dylib path changes in a hidden developer panel.

---

# Appendix C: Merge Implementation Details (Window + Segments) + Pseudocode (v2)

This appendix provides concrete implementation guidance for:
- Timestamp-aware tail extraction.
- Language-aware de-dup.
- Stable tail + rollback.
- Stale-result rejection.

## C.1 Data model (recommended)

```rust
pub struct MergeState {
  // Live refinement state (per in-progress segment)
  stable_tokens: Vec<NToken>,
  // last N candidates (after de-dup), used to compute stable prefix
  recent_candidates: std::collections::VecDeque<Vec<NToken>>,

  // For rollback/stale logic:
  last_applied_job_id: u64,
}

#[derive(Clone, Debug)]
pub struct NToken {
  pub display: String,  // original display text (or minimal unit)
  pub norm: String,     // normalized for matching (lowercased, punctuation stripped if applicable)
  pub kind: TokenKind,  // Word / Grapheme / Char / Punct (optional)
}

#[derive(Copy, Clone, Debug)]
pub enum TokenKind { Word, Grapheme, Char, Punct }

pub struct WindowJobSnapshot {
  pub engine_generation: u64,
  pub window_generation: u64,
  pub job_id: u64,

  pub window_end_16k_samples: u64,
  pub window_len_16k_samples: usize,
  pub context_len_16k_samples: usize,
}
```

## C.2 Window ring buffer (16k) and committed boundary

Maintain:

- `total_16k_samples: u64` (monotonic)
- `window_ring: RingBuffer<f32>` sized to `window_len_16k_samples` (dynamic resizing allowed)
- `committed_end_16k_samples: u64` (monotonic)

When new audio arrives (native):
1. resample to 16k
2. push into ring buffer
3. `total_16k_samples += resampled_len`

When a segment is committed (endpoint or forced-finalize), update committed boundary:

- Simplest approximation: also resample the segment audio to 16k and add its length:
- `committed_end_16k_samples += seg_16k_len`
- Or if you already track `total_16k_samples` at segment boundary, set:
- `committed_end_16k_samples = boundary_total_16k_samples`

## C.3 Window job scheduling + stale rejection

Producer (streaming driver):
- Every `step_ms`, prepare a `WindowJobSnapshot`:
  - `job_id += 1`
  - `window_end_16k_samples = total_16k_samples`
  - Include `engine_generation` and `window_generation`.
- `try_send` to low-priority queue; drop if full.
- Keep at most 1–2 queued ticks.

Consumer (whisper worker):
- Decode and return `(snapshot, decoded_segments_with_timestamps, raw_text)`.
Session updater (apply):
- Accept only if:
- `snapshot.engine_generation == engine_generation_now`
  - `snapshot.window_generation == session.window_generation_now`
  - `snapshot.job_id > merge_state.last_applied_job_id` (or == latest expected)
  - `snapshot.window_end_16k_samples` is not older than current `total_16k_samples` by more than a small tolerance

Otherwise drop result and do not emit.

## C.4 Timestamp-aware tail extraction (preferred)

Assume whisper returns segments with `(t0_ms, t1_ms, text)` relative to the window audio.

Algorithm:

```rust
fn extract_window_tail_text(
  snapshot: &WindowJobSnapshot,
  total_16k_samples_at_snapshot: u64,
  committed_end_16k_samples: u64,
  whisper_segments: &[WhisperSeg], // each has t0_ms, t1_ms, text
) -> String {
  let window_end = total_16k_samples_at_snapshot;
  let window_start = window_end.saturating_sub(snapshot.window_len_16k_samples as u64);

  let cut = committed_end_16k_samples
    .saturating_sub(snapshot.context_len_16k_samples as u64)
    .max(window_start);

  let mut out = String::new();

  for seg in whisper_segments {
    let seg_abs_end = window_start + ms_to_samples_16k(seg.t1_ms);
    if seg_abs_end > cut {
      // Include this segment (or include only its tail; simplest is include full segment text)
      if !out.is_empty() { out.push(' '); } // NOTE: see language-specific joining below
      out.push_str(seg.text.trim());
    }
  }

  out.trim().to_string()
}
```

**Language note:** For CJK, you may prefer joining without spaces. A safe rule:
- If the whisper text already contains spaces (English), keep them.
- If it doesn't (Chinese), don't inject spaces.
(See tokenization and join rules below.)

## C.5 Language-aware tokenization

Use a single function that returns `(tokens, join_rule)`.

Recommended crate for grapheme segmentation:
- `unicode-segmentation`

Tokenization rules:

- If language is English or the text contains whitespace:
  - Split on whitespace into word tokens.
- Normalize: lowercase + strip edge punctuation.
- Else (CJK/no whitespace):
  - Split into grapheme clusters (or chars)  - Normalize: keep as-is or lowercase if alphabetic; optionally strip common punctuation tokens.
Pseudocode:

```rust
fn tokenize(text: &str, lang: Lang) -> (Vec<NToken>, JoinRule) {
  if lang.is_whitespace_language() || text.contains(char::is_whitespace) {
    let toks = text.split_whitespace().map(|w| {
      let display = w.to_string();
      let norm = normalize_word(w);
      NToken { display, norm, kind: TokenKind::Word }
    }).collect();
    (toks, JoinRule::Space)
  } else {
    let toks = unicode_segmentation::UnicodeSegmentation::graphemes(text, true)
      .filter(|g| !g.trim().is_empty())
      .map(|g| {
        let display = g.to_string();
        let norm = normalize_grapheme(g);
        NToken { display, norm, kind: TokenKind::Grapheme }
      })
      .collect();
    (toks, JoinRule::None)
  }
}

enum JoinRule { Space, None }

fn tokens_to_string(tokens: &[NToken], rule: JoinRule) -> String {
  match rule {
    JoinRule::Space => tokens.iter().map(|t| t.display.as_str()).collect::<Vec<_>>().join(" "),
    JoinRule::None => tokens.iter().map(|t| t.display.as_str()).collect::<Vec<_>>().join(""),
  }
}
```

## C.6 Boundary de-dup (short overlap removal)

You already have committed text (segments joined) and window tail text. Remove duplication at the boundary.

Algorithm (token overlap):

```rust
fn dedup_boundary(
  committed_text: &str,
  window_tail_text: &str,
  lang: Lang,
  k_words: usize,
  k_chars: usize,
) -> String {
  let (c_toks, c_join) = tokenize(committed_text, lang);
  let (w_toks, w_join) = tokenize(window_tail_text, lang);

  let k = if matches!(w_join, JoinRule::Space) { k_words } else { k_chars };
  let c_suffix = if c_toks.len() > k { &c_toks[c_toks.len()
- k..] } else { &c_toks[..] };

  // Find longest suffix of c_suffix that matches a prefix of w_toks.
  let mut best = 0usize;
  let max = c_suffix.len().min(w_toks.len());

  for len in 1..=max {
    let c_part = &c_suffix[c_suffix.len()
- len..];
    let w_part = &w_toks[..len];

    if c_part.iter().map(|t| &t.norm).eq(w_part.iter().map(|t| &t.norm)) {
      best = len;
    }
  }

  let w_remain = &w_toks[best..];
  tokens_to_string(w_remain, w_join)
}
```

Fallback behavior:
- If overlap matching is too weak or text is suspicious (e.g., best==0 and window_tail is very long), you may choose:
  - Replace-only tail for that tick (do not attempt append)  - Or fallback to sherpa tail for that tick.

## C.7 Live stability with rollback (rolling N candidates)

**Recommended approach (simple + robust):**
- Keep last N accepted candidate token lists.
- Stable prefix is the LCP across these N candidates.
- Everything after stable prefix is unstable tail.
- Rollback happens naturally when LCP shrinks.

Pseudocode:

```rust
fn lcp_two(a: &[NToken], b: &[NToken]) -> usize {
  let n = a.len().min(b.len());
  for i in 0..n {
    if a[i].norm != b[i].norm { return i; }
  }
  n
}

fn lcp_all(cands: &std::collections::VecDeque<Vec<NToken>>) -> usize {
  if cands.is_empty() { return 0; }
  let mut l = cands[0].len();
  for i in 1..cands.len() {
    l = l.min(lcp_two(&cands[0][..l], &cands[i]));
  }
  l
}

fn apply_stability(
  merge: &mut MergeState,
  candidate_tokens: Vec<NToken>,
  n: usize,
  rollback_threshold: usize,
) -> (Vec<NToken> /*stable*/, Vec<NToken> /*unstable*/) {
  // Push candidate
  merge.recent_candidates.push_back(candidate_tokens);
  while merge.recent_candidates.len() > n {
    merge.recent_candidates.pop_front();
  }

  let stable_len = if merge.recent_candidates.len() == n {
    lcp_all(&merge.recent_candidates)
  } else {
    0 // not enough history yet
  };

  // Rollback policy:
  // If previously stable was longer than new stable by more than rollback_threshold,
  // allow stable to shrink immediately (do not lock in wrong words).
  let prev_stable_len = merge.stable_tokens.len();
  let new_stable_len = stable_len;

  if prev_stable_len > new_stable_len + rollback_threshold {
    // rollback: accept the shorter stable prefix
  }

  // Use most recent candidate to define current stable/unstable split
  let last = merge.recent_candidates.back().unwrap();
  let stable = last[..new_stable_len].to_vec();
  let unstable = last[new_stable_len..].to_vec();

  merge.stable_tokens = stable.clone();
  (stable, unstable)
}
```

Notes:
- This method is deterministic and naturally limits rewrites to the tail.
- When an endpoint occurs, reset:
  - `stable_tokens = []`
  - `recent_candidates.clear()`

## C.8 Putting it together: window result → live text update

```rust
fn merge_window_into_live_text(
  session: &mut SessionInner,
  merge: &mut MergeState,
  lang: Lang,
  snapshot: WindowJobSnapshot,
  whisper_segments: Vec<WhisperSeg>,
) -> Option<String> {
  // 1) stale checks
  if snapshot.window_generation != session.window_generation { return None; }
  if snapshot.job_id <= merge.last_applied_job_id { return None; }
  merge.last_applied_job_id = snapshot.job_id;

  // 2) extract tail from timestamps
  let tail_raw = extract_window_tail_text(
    &snapshot,
    snapshot.window_end_16k_samples,
    session.committed_end_16k_samples,
    &whisper_segments,
  );

  if tail_raw.trim().is_empty() {
    return None; // do not spam empty updates
  }

  // 3) boundary de-dup vs committed segments
  let committed_text = session.stt_segments.join(" ");
  let tail_dedup = dedup_boundary(
    &committed_text,
    &tail_raw,
    lang,
    session.settings.merge.overlap_k_words,
    session.settings.merge.overlap_k_chars,
  );

  if tail_dedup.trim().is_empty() {
    return None;
  }

  // 4) stability
  let (cand_toks, join_rule) = tokenize(&tail_dedup, lang);
  let (stable, unstable) = apply_stability(
    merge,
    cand_toks,
    session.settings.merge.stability_n,
    session.settings.merge.rollback_threshold_tokens,
  );

  let live_text = {
    let mut s = tokens_to_string(&stable, join_rule);
    let u = tokens_to_string(&unstable, join_rule);
    if !s.is_empty() && !u.is_empty() && matches!(join_rule, JoinRule::Space) {
      s.push(' ');
    }
    s.push_str(&u);
    s.trim().to_string()
  };

  Some(live_text)
}
```

## C.9 Emission gating and revision

When applying a new live text:

- If `live_text` is empty/whitespace, do not emit.
- If `live_text` is unchanged and committed segments unchanged, do not emit.
- Otherwise:
  - `session.stt_live_text = live_text`
  - `session.stt_revision += 1`
  - Emit `stt/partial { revision, text: build_stt_text(), strategy: sliding_window }`.
---

# Appendix D: UI Guidance for "No App Restart" Experience

- Normal settings UI should only include knobs that are `SoftApplied` or `Reloaded`.
- Any `RestartRequired` knob must be hidden behind a developer toggle and clearly labeled.
- When applying `Reloaded`, UI shows:
  - "Loading speech engine…" + spinner
  - Disables PTT until `engine/state=ready`.
This ensures user-perceived experience is smooth and never requires quitting/reopening the app.
