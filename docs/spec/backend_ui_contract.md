# Backend and UI Contract (FFI)

This document defines the C ABI contract between the SwiftUI macOS app and the Rust speech engine.

The Rust engine is the single source of truth for transcription events. The SwiftUI app must not implement its own recognition logic.

## Header location

- `crates/inkflow-ffi/include/inkflow.h`
- The SwiftUI app includes it via the bridging header `apps/macos/InkFlow/InkFlow/InkFlowBridge.h`.

## Status codes

`inkflow.h` defines these status codes:

- `INKFLOW_OK` = 0
- `INKFLOW_ERR_NULL` = 1
- `INKFLOW_ERR_INVALID_ARGUMENT` = 2
- `INKFLOW_ERR_INTERNAL` = 3

## C ABI functions

```c
InkFlowHandle *inkflow_engine_create(void);
void inkflow_engine_destroy(InkFlowHandle *handle);

int32_t inkflow_engine_submit_audio(
  InkFlowHandle *handle,
  const float *samples,
  size_t sample_count,
  uint32_t sample_rate_hz
);

int32_t inkflow_engine_register_callback(
  InkFlowHandle *handle,
  inkflow_update_cb callback,
  void *user_data
);

void inkflow_engine_unregister_callback(InkFlowHandle *handle);
```

## Audio input contract

- Format: mono `float32` PCM.
- Sample rate: device sample rate, passed on every submission.
- Sample rate must remain constant for the session.
- The Rust engine resamples to 16 kHz internally for sherpa-onnx and whisper.

## Callback contract

- `inkflow_update_cb` is invoked on a **background thread**.
- The `utf8` string is valid only for the duration of the callback.
- The SwiftUI app must dispatch UI updates onto the main thread.

### Update payload format (JSON)

The callback delivers JSON strings. Each payload has a `kind` and optional fields.

Common shape:

```json
{
  "kind": "sherpa_partial",
  "text": "hello world"
}
```

#### `sherpa_partial`

```json
{ "kind": "sherpa_partial", "text": "partial text" }
```

#### `window_result`

```json
{
  "kind": "window_result",
  "snapshot": {
    "engine_generation": 1,
    "window_generation": 2,
    "job_id": 3,
    "window_end_16k_samples": 64000,
    "window_len_16k_samples": 64000,
    "context_len_16k_samples": 12800
  },
  "result": {
    "text": "window transcript",
    "has_timestamps": true,
    "segments": [
      { "t0_ms": 0, "t1_ms": 1200, "text": "Hello" }
    ]
  }
}
```

#### `segment_end`

```json
{
  "kind": "segment_end",
  "segment_id": 5,
  "text": "sherpa segment",
  "committed_end_16k_samples": 96000,
  "window_generation_after": 4
}
```

#### `second_pass`

```json
{ "kind": "second_pass", "segment_id": 5, "text": "whisper replacement" }
```

#### `endpoint_reset`

```json
{ "kind": "endpoint_reset", "window_generation_after": 5 }
```

#### `error`

```json
{ "kind": "error", "code": "stt_decode_failed", "message": "..." }
```
