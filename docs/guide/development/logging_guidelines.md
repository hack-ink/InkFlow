# Logging Guidelines

This document defines logging levels and conventions for InkFlow.

## Level policy

- **Error**: The operation failed and user-facing behavior is degraded. Emit when a request or
  worker cannot proceed, when an external dependency fails, or when a fallback is required.
- **Warn**: The operation continues but with degraded behavior or data loss. Examples include
  queue drops, backpressure suppressing decoding, or unexpected state transitions.
- **Info**: Stable, low-frequency lifecycle events. Use for startup, configuration summaries,
  segment commits, and successful second-pass replacements.
- **Debug**: Diagnostic details and high-frequency events. Use for live render updates, gating
  decisions, queue enqueue/dequeue events, and detailed metrics.
- **Trace**: Extremely high-frequency instrumentation, usually disabled outside targeted
  profiling sessions.

## Message conventions

- Use clear, complete English sentences.
- Include structured fields for IDs (segment_id, window_generation) and numeric metrics.
- Avoid logging raw audio or long transcripts. Prefer short previews when necessary.
- Log only when state changes; avoid repeating the same message every tick.

## STT pipeline examples

- **Info**
  - STT pipeline initialized.
  - Segment committed.
  - Second-pass transcription delivered.
- **Warn**
  - Second-pass queue dropped segments.
  - Window decoding suppressed due to backpressure.
- **Debug**
  - Window activity gate suppressed/resumed decoding.
  - Second-pass enqueued/dequeued.
  - Render update emitted (with preview).

## Debugging workflow

To enable diagnostic logs for the render pipeline without flooding other modules:

```bash
RUST_LOG=inkflow_core::engine::render=debug,inkflow_core=info
```

For deeper analysis, raise the level for the specific module under investigation and keep
other modules at info or warn.

## Retention

Log files are automatically pruned after seven days. The current log file is always retained.
