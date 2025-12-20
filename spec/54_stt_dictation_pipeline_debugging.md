# STT Dictation v2 – Debugging and Reproducible Test Workflow

Date: 2025-12-17

This document describes how to produce reproducible artifacts (audio + structured event traces) for dictation v2 issues such as:

- Flicker or “module fighting” between sherpa partials, whisper sliding-window updates, and whisper second-pass replacements.
- Unexpected transcript rewrites around endpoints/finalization.
- Hallucinated text during long silence while still listening.
- Sensitivity to speech rate (fast vs slow, long pauses).

The goal is to turn subjective reports into shareable, replayable evidence.

## 1. Enable a structured STT trace in the app

The backend can write an NDJSON trace and the recorded session audio when tracing is enabled.

By default, whisper.cpp / GGML logs are silenced to keep traces readable. To re-enable them for investigation, set `AIR_WHISPER_LOGS=1`.

Enable tracing with one of the following:

- `AIR_STT_TRACE=1` (writes into `tmp/stt_trace/`)
- `AIR_STT_TRACE_DIR=/absolute/or/relative/path` (custom directory)

Run the app from a terminal so logs and trace files are available:

- `cargo make dev`
- `cargo make dev-trace` (enables trace and writes into `tmp/stt_trace/` at the repository root)

Note: `tmp/stt_trace/` is resolved relative to the process working directory. In the `cargo make dev` workflow this is typically under `src-tauri/`, so the default location is often `src-tauri/tmp/stt_trace/`. The backend prints the resolved absolute trace directory on session start (look for `STT trace enabled. Directory: ...`).

After a dictation session is finalized (`Enter`), look for:

- `tmp/stt_trace/stt_session_<SESSION_ID>.ndjson`
- `tmp/stt_trace/stt_session_<SESSION_ID>.wav`

If the session is cancelled (`Escape` or losing focus), the WAV file is still written when tracing is enabled.

### 1.1 macOS: trace WAV is silent

If `stt_session_<ID>.wav` is completely silent (all zeros) even when you are speaking, check your system audio routing:

- `system_profiler SPAudioDataType` (look for `Default Input Device` and `Default Output Device`).

Some audio capture backends can accidentally bind to the default output device. If your default output is an external output-only device (for example a USB DAC), microphone capture may produce all-zero samples.

To confirm quickly, run the comparison harness and look at the reported microphone stats:

- `cargo make stt-compare-mic`

It prints `mic_stats mean_abs=... peak_abs=...`. If `peak_abs` stays `0.000000` while speaking, the capture pipeline is not receiving microphone audio.

## 2. What the trace contains

The trace file is JSON Lines (one JSON object per line). Each line includes:

- `t_ms`: milliseconds since session start.
- `kind`: event category (`stt_partial`, `segment_commit`, `window_scheduled`, `endpoint_reset`, `stt_final`, etc.).
- `revision`: present for `stt_partial` lines.
- `strategy`: present for `stt_partial` lines (`vad_chunk` or `sliding_window`).
- `text`: the full transcript text (matching the UI’s `stt/partial` payload) when applicable.
- `details`: session-local counters (window generation, last scheduled/applied job id, and whether window text is active).

This is intended to make “who changed the text, when, and why” explicit.

Note: `segment_commit` lines should not have an empty `text` value. If you observe empty segment commits, treat it as a bug and include the trace as a regression case.
See `spec/55_stt_dictation_trace_regressions.md` for known regression cases.

## 3. Build a small, targeted reproduction set

Create a few short recordings (5–15 seconds each). Keep them minimal and label them clearly.

Recommended cases:

1. **Fast speech, no long pauses**
2. **Slow speech, long pauses between words**
3. **Stop speaking but keep holding Space for 3–5 seconds**
4. **Speak, then immediately release Space**
5. **Quiet room silence (hold Space, say nothing for 5 seconds)**

For each case, collect:

- The `.wav` file.
- The `.ndjson` trace.
- A short note describing what you expected vs what happened.

Avoid committing large audio files to the repository; store them locally and share as needed.

## 4. Use the comparison harness on captured audio (optional)

The workspace includes `stt-compare`, which can help separate engine behavior from merge behavior:

- `cargo make stt-compare -- --manifest research/stt_compare/manifest.toml`

You can also run it on a single WAV by editing/creating a local manifest that points to the captured `.wav`.

## 5. How to report an issue with minimal ambiguity

When reporting, include:

- The trace and WAV file names.
- The `kind` sequence around the problematic moment (timestamps).
- The last 3–5 `stt_partial` lines before/after the rewrite.
- Which behavior category it matches (flicker, finalization regression, silence hallucination, speech rate sensitivity).
- A trace summary to highlight strategy switches and rewrites:
  - `python3 script/stt_trace_summary.py --path tmp/stt_trace/stt_session_X.ndjson --timeline`
  - `python3 script/stt_trace_summary.py --path tmp/stt_trace/stt_session_X.ndjson --events`

If you start a new session before the previous finalization completes, the previous session may emit a `finalize_detached` trace line. In that case, the WAV file is still written (when tracing is enabled), but the previous session may not produce a complete `stt_final` line.
