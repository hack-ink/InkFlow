# AiR

AiR is a macOS 13+ “SuperWhisper-like” floating voice input assistant built with Rust + Tauri v2.

## Development

Note: This repository includes the upstream `third_party/sherpa-onnx` git submodule. You can either clone with submodules (`--recurse-submodules`) or run `cargo make setup-macos`, which will initialize/update the submodule automatically.
The generated native libraries are installed under `third_party/sherpa-onnx-prefix/` and are intentionally not committed.

Related docs:

- `spec/50_speech_to_text.md`: sherpa-onnx build + runtime notes.
- `spec/52_stt_dictation_pipeline_spec.md`: Canonical STT pipeline spec (two-pass + sliding-window refinement).
- `spec/56_stt_comparison_harness.md`: STT A/B harness usage.

Build sherpa-onnx native libraries and download the default model (macOS):

```sh
cargo make setup-macos
```

Run the STT comparison harness (sample manifest):

```sh
cargo make stt-compare
```

## Rust checks (Repository Rule)

Use `cargo make` tasks:

```sh
cargo make fmt
cargo make clippy
cargo make nextest
```
