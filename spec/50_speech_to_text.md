# Speech-to-Text

This document describes the speech-to-text integration used by AiR (sherpa-onnx streaming partials and a whisper second pass).

## Goals

- Low-latency streaming dictation (partial results while speaking).
- Fully local/offline recognition (no Internet required at runtime).
- Performance-first defaults (int8 where practical).
- Minimal native build (C API only; disable unused components).
- Two-pass finalization: sherpa streaming partials + whisper final text after each endpoint.
- Optional live refinement (planned): whisper sliding-window decoding for higher-quality partial text (canonical spec: `spec/52_stt_dictation_pipeline_spec.md`).

## Quick Start (macOS)

Run:

```sh
cargo make setup-macos
cargo make dev
```

No environment variables are required for the default repository layout. The app will automatically locate:

- Native libraries in `third_party/sherpa-onnx-prefix/lib/`.
- The default model in `model/sherpa-onnx-streaming-zipformer-en-2023-06-21/`.
- The default whisper model in `model/whisper/ggml-large-v3-turbo-q8_0.bin` (or `AIR_WHISPER_MODEL_PATH`).

The `third_party/sherpa-onnx-prefix/` directory is a generated build artifact and is ignored by git.

## Default Model (Streaming English)

Default model directory name:

- `sherpa-onnx-streaming-zipformer-en-2023-06-21`

Default file selection (performance-first):

- `tokens.txt`
- `encoder-epoch-99-avg-1.int8.onnx`
- `joiner-epoch-99-avg-1.int8.onnx`
- `decoder-epoch-99-avg-1.onnx` (fp32 by default)

## Runtime Configuration (Environment Variables)

These environment variables are optional overrides. AiR will use executable-relative defaults when they are not provided.

- `AIR_SHERPA_ONNX_MODEL_DIR`
  - Path to the model directory containing `tokens.txt` and encoder/decoder/joiner ONNX files.
  - Default: auto-discovered (typically `model/sherpa-onnx-streaming-zipformer-en-2023-06-21`).
- `AIR_SHERPA_ONNX_PROVIDER`
  - ONNX Runtime provider string. Typical values: `cpu`, `coreml`.
  - Default: `cpu`
- `AIR_SHERPA_ONNX_NUM_THREADS`
  - Number of CPU threads used by sherpa-onnx.
  - Default: half of the available CPU cores.
- `AIR_SHERPA_ONNX_DECODING_METHOD`
  - Decoding method used by the streaming recognizer.
  - Valid values: `greedy_search`, `modified_beam_search`.
  - Default: `greedy_search`
- `AIR_SHERPA_ONNX_MAX_ACTIVE_PATHS`
  - Beam width for `modified_beam_search`.
  - Default: `4`
- `AIR_SHERPA_ONNX_RULE1_MIN_TRAILING_SILENCE`
  - Endpoint rule 1: minimum trailing silence, in seconds.
  - Default: `2.4`
- `AIR_SHERPA_ONNX_RULE2_MIN_TRAILING_SILENCE`
  - Endpoint rule 2: minimum trailing silence, in seconds.
  - Default: `1.2`
- `AIR_SHERPA_ONNX_RULE3_MIN_UTTERANCE_LENGTH`
  - Endpoint rule 3: minimum utterance length, in seconds.
  - Default: `300.0`
- `AIR_SHERPA_ONNX_PREFER_INT8`
  - When `true`, prefer int8 encoder/joiner when available.
  - Default: `true`
- `AIR_SHERPA_ONNX_USE_INT8_DECODER`
  - When `true`, use `decoder-epoch-99-avg-1.int8.onnx` if available.
  - Default: `false`
- `AIR_SHERPA_ONNX_DYLIB`
  - Overrides the dynamic library name/path loaded by the Rust wrapper (useful for custom install paths).
  - Default: auto-discovered.
  - Note: You can also rely on `DYLD_LIBRARY_PATH`, but `AIR_SHERPA_ONNX_DYLIB` is the most explicit option.

## Two-pass Finalization (Whisper Second Pass)

During dictation, AiR uses sherpa-onnx streaming to produce low-latency partial text and to detect endpoints. After each endpoint, AiR transcribes the corresponding audio segment using whisper-rs and replaces the provisional sherpa segment text with the whisper output.

To avoid repeated “empty endpoints” during silence, segments with an empty sherpa transcript are ignored, and the whisper second pass is skipped for near-silent audio.

### Default Whisper Model

- Default model path: `model/whisper/ggml-large-v3-turbo-q8_0.bin`
- Override: `AIR_WHISPER_MODEL_PATH=/absolute/path/to/model.bin`

The default path is auto-discovered relative to the running executable:

- macOS app bundle: `Contents/Resources/model/whisper/ggml-large-v3-turbo-q8_0.bin`
- Repo layout: `model/whisper/ggml-large-v3-turbo-q8_0.bin`

### Whisper Runtime Configuration (Environment Variables)

- `AIR_WHISPER_MODEL_PATH`
  - Path to the whisper GGML model file.
  - Default: auto-discovered (typically `model/whisper/ggml-large-v3-turbo-q8_0.bin`).
- `AIR_WHISPER_LANGUAGE`
  - Whisper language code (for example, `en`). Use `auto` to enable language detection.
  - Default: `en`
- `AIR_WHISPER_NUM_THREADS`
  - Whisper thread count for decoding.
  - Default: whisper-rs default.
- `AIR_WHISPER_FORCE_GPU`
  - When set, forces whisper GPU usage on or off (`true`/`false`).
  - Default: whisper-rs default.

## Model Download (Reference)

```bash
mkdir -p model
cd model
wget https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-streaming-zipformer-en-2023-06-21.tar.bz2
tar xvf sherpa-onnx-streaming-zipformer-en-2023-06-21.tar.bz2
rm sherpa-onnx-streaming-zipformer-en-2023-06-21.tar.bz2
```

## Native Library Build (macOS, Minimal C API)

The Rust code uses the sherpa-onnx C API via dynamic loading. AiR expects these libraries to exist (by default under `third_party/sherpa-onnx-prefix/lib/`):

- `libsherpa-onnx-c-api.dylib`
- `libonnxruntime.dylib` (dependency of the C API library)

### Automated Build (Recommended)

Use the repository task:

```sh
cargo make setup-macos
```

This will:

- Ensure `third_party/sherpa-onnx` is available (git submodule when present; otherwise a direct clone).
- Build and install a minimal C API-only configuration into `third_party/sherpa-onnx-prefix`.
- Rewrite `third_party/sherpa-onnx-prefix/sherpa-onnx.pc` to use a relocatable `prefix=${pcfiledir}`.
- Download and extract the default streaming model into `model/` if missing.

### Manual Build (Reference)

Recommended build flags (disable unused components):

```bash
export SHERPA_ONNX_PREFIX="$PWD/third_party/sherpa-onnx-prefix"

cd third_party
git clone https://github.com/k2-fsa/sherpa-onnx
cd sherpa-onnx

cmake -S . -B build-macos-min -GNinja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX="$SHERPA_ONNX_PREFIX" \
  -DBUILD_SHARED_LIBS=ON \
  -DSHERPA_ONNX_ENABLE_C_API=ON \
  -DSHERPA_ONNX_ENABLE_PORTAUDIO=OFF \
  -DSHERPA_ONNX_ENABLE_WEBSOCKET=OFF \
  -DSHERPA_ONNX_ENABLE_BINARY=OFF \
  -DSHERPA_ONNX_BUILD_C_API_EXAMPLES=OFF \
  -DSHERPA_ONNX_ENABLE_TTS=OFF \
  -DSHERPA_ONNX_ENABLE_SPEAKER_DIARIZATION=OFF \
  -DSHERPA_ONNX_ENABLE_PYTHON=OFF \
  -DSHERPA_ONNX_ENABLE_JNI=OFF \
  -DSHERPA_ONNX_ENABLE_TESTS=OFF \
  -DSHERPA_ONNX_ENABLE_WASM=OFF \
  -DSHERPA_ONNX_ENABLE_GPU=OFF \
  -DSHERPA_ONNX_ENABLE_DIRECTML=OFF \
  -DSHERPA_ONNX_ENABLE_RKNN=OFF \
  -DSHERPA_ONNX_ENABLE_AXERA=OFF \
  -DSHERPA_ONNX_ENABLE_AXCL=OFF \
  -DSHERPA_ONNX_ENABLE_ASCEND_NPU=OFF \
  -DSHERPA_ONNX_ENABLE_QNN=OFF \
  -DSHERPA_ONNX_ENABLE_SPACEMIT=OFF

cmake --build build-macos-min
cmake --install build-macos-min
```

## Repository Integration Notes

- `crates/sherpa-onnx-sys` generates bindings at build time from `vendor/sherpa_onnx_c_api.h`.
- `crates/sherpa-onnx` loads the native library at runtime (`libloading`) and exposes:
  - `OnlineRecognizer`
  - `OnlineStream`
  - `OnlineResult` parsed from the C API JSON output.
- Endpointing defaults match the upstream streaming microphone demo:
  - rule1=2.4s, rule2=1.2s, rule3=300.0s.
- Microphone capture defaults are tuned to match the upstream demo:
  - Prefer 48 kHz input when supported, downmix to mono, clamp to `[-1.0, 1.0]`, and feed ~100 ms chunks.
- Dynamic library lookup order (macOS):
  - `AIR_SHERPA_ONNX_DYLIB` (if set).
  - App bundle `Contents/Frameworks/` (if packaged as `.app`).
  - Executable directory (development convenience).
  - `third_party/sherpa-onnx-prefix/lib/` (repo default).
  - Platform dynamic linker fallback.

## Related Documents

- For a quick A/B comparison between "two-pass" and "whisper-only" baselines, see `spec/56_stt_comparison_harness.md`.
- For the canonical dictation pipeline spec (including UI stability and overlap removal), see `spec/52_stt_dictation_pipeline_spec.md`.
