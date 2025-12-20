# STT Comparison Harness (Two-pass vs Whisper-only)

This document describes how to quickly compare two candidate dictation pipelines:

1. Two-pass: sherpa-onnx streaming for low-latency partials + whisper (turbo) for final text after each endpoint.
2. Whisper-only: a naive rolling-window decode loop to simulate partials (baseline for feasibility).

The goal is to make a fast, repeatable decision with measurable latency and visible transcript differences.
It is also useful for regression checks when changing merge or endpoint behavior.

## Tooling

The repository includes a small Rust CLI:

- `research/stt_compare` (`stt-compare`)

It supports manifest-driven WAV input and microphone input. Each run executes both the two-pass pipeline and the whisper rolling-window baseline for the same input and prints a single combined report.

## Quick Start

1. Build sherpa-onnx C API and download the default streaming model:

```sh
cargo make setup-macos
```

This will also initialize/update the `third_party/sherpa-onnx` git submodule when present.

2. Run the comparison on the sample audio:

```sh
cargo make stt-compare
```

3. Microphone runs (stop with Enter):

```sh
cargo make stt-compare-mic
```

## Two-pass pipeline output (`mode=twopass`)

Two-pass mode uses sherpa endpointing to segment the audio. Each segment is then decoded by whisper and printed as the final text.

Recommended accuracy-oriented sherpa settings (optional):

```sh
cargo run -p stt-compare --release -- --manifest research/stt_compare/manifest.toml \
  --sherpa-provider coreml \
  --sherpa-prefer-fp32 \
  --sherpa-decoding-method modified_beam_search \
  --sherpa-max-active-paths 8
```

Microphone input:

```sh
cargo run -p stt-compare --release -- --mic --print-partials
```

Interpretation:

- Each segment prints both the sherpa text (streaming) and the whisper text (final).
- The printed whisper latency approximates "endpoint → final" time for that segment.
- Whisper language defaults to `en`. Use `--whisper-language auto` or `--whisper-language <code>` when needed.

## Whisper rolling-window output (`mode=whisper-window`)

Whisper rolling-window mode repeatedly decodes the last `window_ms` of audio every `step_ms`. This is not a production streaming solution, but it provides a practical baseline for:

- How often whisper can be run before it stops keeping up with real time.
- How stable the partial text looks when repeatedly decoding a moving window.

Example (show partial changes):

```sh
cargo run -p stt-compare --release -- --manifest research/stt_compare/manifest.toml \
  --whisper-window-ms 8000 \
  --whisper-step-ms 200 \
  --print-partials
```

Microphone input:

```sh
cargo run -p stt-compare --release -- --mic --print-partials
```

Interpretation:

- `tick budget ratio` > 1.0 means the approach cannot keep up at that `step_ms` cadence on your machine.
- The final transcript is always decoded on the full audio at the end.

## Microphone Notes (macOS)

- If you denied microphone permission, re-enable it in System Settings > Privacy & Security > Microphone.
- The tool uses the default microphone configuration and prints the selected sample rate and channel configuration.
- Microphone input is recorded until you press Enter.

## Output Format

The CLI prints stable, grep-friendly lines:

- `[config] ...` for runtime configuration.
- `[tick] ...` for whisper-window progress (includes a transcript snippet).
- `[partial] ...` for sherpa partial updates (only when `--print-partials` is enabled).
- `[seg] ...` for twopass endpoint segments (prints both sherpa and whisper text).
- `[final] ...` for final transcripts.
- `[accuracy] ...` for WER/CER when a reference transcript is available.
- `[summary] ...` for timing statistics.

All time fields are reported in milliseconds.

## Reference Transcripts and Accuracy (WER/CER)

To score accuracy, provide an input manifest (which also lists the WAV files to run):

- `--manifest path/to/manifest.toml` to select the manifest.

If `--manifest` is not provided and `research/stt_compare/manifest.toml` exists, the tool loads it automatically.

For each `[[references]]` entry:

- `wav` may be absolute or relative. Relative paths are resolved relative to the manifest directory (then the current working directory).
- `text` may be empty to disable WER/CER scoring.
