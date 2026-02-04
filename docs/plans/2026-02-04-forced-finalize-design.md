# Forced Finalize for Explicit Stop (Design)

## Summary
Implement an explicit forced-finalize path that runs when the user stops dictation. The microphone stops immediately, and the engine force-flushes the current buffered speech to ensure the last segment is committed. The update order remains unchanged: `segment_end` may arrive before `second_pass`, preserving the existing fast-then-refine experience.

## Goals
- Ensure the final spoken segment is not dropped when the user explicitly stops dictation.
- Keep the user experience unchanged: instant stop, immediate provisional output, later second-pass refinement.
- Avoid new timing knobs or arbitrary delays in the UI.

## Non-goals
- Redesign update ordering to wait for second-pass before `segment_end`.
- Change dictation hotkey behavior or mode selection.
- Add new UI affordances.

## Architecture
Add a control command to the streaming worker to trigger forced finalize without closing the audio channel. Expose it through the Rust engine API and the FFI layer. The macOS app calls the new API on explicit stop actions.

### Data Flow
1. User releases the dictation hotkey or presses the stop control.
2. macOS app stops microphone capture immediately.
3. The app calls `force_finalize()` on the Rust engine.
4. The stream worker flushes buffered audio using the existing finalize logic and emits `segment_end` if there is valid speech.
5. Second-pass jobs proceed as today and may emit `second_pass` updates later.

### Worker Behavior
The stream worker accepts a new `Finalize` command. On receipt, it:
- Pads tail audio and calls `input_finished()` on the sherpa stream.
- Decodes and emits a `segment_end` update if there is valid speech.
- Schedules second-pass work using the existing scheduler.
- Resets segment state and the sherpa stream so the pipeline can continue on a new session.

## API and FFI Changes
- `InkFlowEngine::force_finalize()` is added to the Rust engine.
- The FFI exposes a new C API call, `inkflow_engine_force_finalize`.
- Swift wraps the call in `InkFlowClient.forceFinalize()`.

## UI Behavior
The UI keeps the current behavior:
- `segment_end` updates may appear immediately.
- `second_pass` updates may replace the segment later.
- `live_render` continues to be driven by the backend render state.

## Error Handling
- If forced finalize sees no valid speech, the engine emits `endpoint_reset`.
- The UI treats `endpoint_reset` as a stop completion signal.

## Risks
- If the engine fails to emit either `segment_end` or `endpoint_reset`, the callback may remain registered. This is tracked as a follow-up issue.

## Testing
- Add unit tests for the new command handling path in the stream worker if practical.
- Add a Rust-level test to ensure `force_finalize` emits `segment_end` or `endpoint_reset` based on buffered audio.
