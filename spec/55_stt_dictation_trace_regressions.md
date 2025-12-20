# Dictation v2 Trace Regressions (Sessions 2–7)

This note captures a reproducible Dictation v2 failure mode observed in `tmp/stt_trace/` and the fixes applied in code.

## Summary

Observed symptoms:

- A phantom leading word appears (for example, `"So"` or a duplicated `"walking"`).
- The transcript looks like multiple recognizers are overwriting each other.

Root cause (trace-driven):

- A very early Sherpa endpoint can produce an **empty or low-quality first segment**.
- That segment can later be **filled by Whisper second pass** with a plausible but incorrect leading token.
- Subsequent segments often contain the real utterance, so the final text ends up with an extra leading word or duplication across segment boundaries.

## Evidence (Provided Trace)

Directory: `tmp/stt_trace/`

- Session 2 reference: `Walking is a great way to unwind. Where do you usually go for walks?`
  - `stt_final` had a duplicated leading word: `walking walking ...`
  - Trace shows an early endpoint committing a short/incorrect first segment, followed by second-pass replacement.
- Session 3 reference: `Sometimes I go to the countryside for more scenic routes. How about you?`
  - `stt_final` had an extra leading token: `So Sometimes ...`
  - Trace shows an empty first segment commit, then second-pass replacement injecting `"So"`.
- Session 4 reference: `I love walking along the riverbank in the city. It's peaceful, especially early in the morning.`
  - Largely correct, with minor tokenization differences (`riverbank` vs `river bank`).

## Additional Evidence (New Trace Round)

Directory: `tmp/stt_trace/`

Reference sentence (multiple sessions): `Usually, I go alone. It's my time to think and relax. But sometimes I go with a friend.`

Observed symptoms:

- Whisper sliding-window can emit short hallucinations early (for example, `"Use your hands."` or `"- Enjoy."`) before stable speech is detected.
- Whisper second-pass can sometimes produce an overly-short replacement (for example, replacing `"I GO ALONE"` with `"alone"`), which can then cascade into incorrect overlap de-duplication across segment boundaries.

Working hypothesis:

- Input audio quality and noise floor strongly influence both window hallucination rate and second-pass truncation risk.
- The existing overlap de-duplication is correct for true cross-segment duplication, but it can remove legitimate tokens when a previous segment ends with a common phrase prefix (for example, `"I go"`) and the next segment begins with the same prefix.

## Fixes Implemented

Code anchor: `src-tauri/src/session.rs`.

1. Empty endpoints are ignored.
   - Endpoints with empty Sherpa text are treated as `EndpointReset` and do not create a committed segment.
2. Whisper window scheduling is gated on Sherpa activity.
   - Window jobs are not scheduled until Sherpa has produced at least one non-empty partial in the current segment.
3. Long leading words from the window are allowed when they match the Sherpa tail.
   - If the window live text equals `(<long word> + sherpa_text)` by token normalization, the window text is used for the provisional segment commit.
   - Short leading tokens (for example, `"So"`) are not accepted by this heuristic.
4. Suspicious second-pass truncations are rejected.
   - Replacements that collapse a multi-word segment into a single-word suffix are ignored and traced as `second_pass_rejected`.
5. Segment boundary de-duplication is applied consistently.
   - When committing a segment and when applying a Whisper second-pass replacement, the segment text is de-duplicated against already-committed text using the existing overlap logic.
6. Leading duplicate tokens from second pass are collapsed.
   - If the second-pass text begins with a repeated long word (for example, `"walking walking ..."`), one copy is removed.
7. Sherpa is always fed 16 kHz audio.
   - This avoids Sherpa’s internal “Creating a resampler” log spam and removes redundant resampling.
8. Microphone capture uses Voice Processing I/O on macOS.
   - This improves input signal quality and reduces window hallucinations compared to raw capture.

## How To Reproduce

1. Run: `cargo make dev-trace`.
2. Record a few PTT sessions.
3. Inspect `tmp/stt_trace/stt_session_*.ndjson` using: `python3 script/stt_trace_summary.py --path tmp/stt_trace/stt_session_X.ndjson --timeline`.
   - For a full event timeline, use `--events`: `python3 script/stt_trace_summary.py --path tmp/stt_trace/stt_session_X.ndjson --events`.
4. If you need a capture workflow reference, see `spec/54_stt_dictation_pipeline_debugging.md`.

## Regression Checks

There are targeted unit tests in `src-tauri/src/session.rs` under `#[cfg(test)]` to cover:

- Leading duplicate collapse.
- Prefix matching used to reject obviously unrelated early second-pass insertions.
- Tail overlap removal between committed text and new segment text.
