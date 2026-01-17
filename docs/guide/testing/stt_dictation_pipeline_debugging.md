# STT Dictation – Debugging Workflow

Date: 2026-01-16

This document describes the current debugging workflow for the SwiftUI + Rust FFI stack.
The pipeline specification remains in `docs/spec/core/stt_dictation_pipeline.md`.

## 1. Run the macOS App Under Xcode

- Open `apps/macos/InkFlow/InkFlow.xcodeproj`.
- Select the `InkFlow` scheme.
- Run with Cmd+R to attach the debugger and view logs.

If you need environment variables, add them under:

- Product → Scheme → Edit Scheme… → Run → Arguments → Environment Variables.

## 2. Smoke Test the Rust Engine (No UI)

This test runs the engine directly and prints the precise initialization error without the UI layer.
It is opt-in to avoid breaking default `cargo test` runs.

```sh
INKFLOW_STT_SMOKE_TEST=1 cargo test -p inkflow-core --test stt_smoke -- --nocapture
```

## 3. Capture Audio for Offline Analysis

The most reliable way to reproduce transcription issues today is to capture audio and run it through the CLI harness.

1. Record a short WAV file (5–15 seconds). Keep the sample clean and label it clearly.
2. Run the comparison harness:

```sh
cargo make stt-compare -- --manifest research/stt_compare/manifest.toml
```

This separates model behavior from UI rendering and makes regressions easier to spot.

## 4. Status of Structured Tracing

Structured trace capture is not yet wired into the SwiftUI + FFI pipeline.
If trace capture is added later, this document will be updated with the new workflow.
