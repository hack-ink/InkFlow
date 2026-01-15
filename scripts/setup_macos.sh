#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This setup script only supports macOS." >&2
  exit 1
fi

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "Missing required command: $cmd." >&2
    exit 1
  fi
}

require_cmd git
require_cmd cmake
require_cmd tar
require_cmd curl
require_cmd sed

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

SHERPA_ONNX_COMMIT="a6a36e8cef858aa4f252f037a94463f20cc48d62"
SHERPA_ONNX_URL="https://github.com/k2-fsa/sherpa-onnx"

THIRD_PARTY_DIR="$ROOT_DIR/third_party"
SHERPA_REPO_DIR="$THIRD_PARTY_DIR/sherpa-onnx"
SHERPA_PREFIX_DIR="$THIRD_PARTY_DIR/sherpa-onnx-prefix"

SHERPA_LIB_DIR="$SHERPA_PREFIX_DIR/lib"
SHERPA_C_API_DYLIB="$SHERPA_LIB_DIR/libsherpa-onnx-c-api.dylib"
ORT_DYLIB="$SHERPA_LIB_DIR/libonnxruntime.dylib"

MODEL_NAME="sherpa-onnx-streaming-zipformer-en-2023-06-21"
MODEL_DIR="$ROOT_DIR/models/$MODEL_NAME"
MODEL_TARBALL="$ROOT_DIR/models/$MODEL_NAME.tar.bz2"
MODEL_URL="https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/$MODEL_NAME.tar.bz2"

WHISPER_MODEL_NAME="ggml-large-v3-turbo-q8_0.bin"
WHISPER_MODEL_DIR="$ROOT_DIR/models/whisper"
WHISPER_MODEL_PATH="$WHISPER_MODEL_DIR/$WHISPER_MODEL_NAME"

echo "Setting up sherpa-onnx (C API) and the default streaming model."

mkdir -p "$THIRD_PARTY_DIR"

is_submodule=false
if [[ -f "$ROOT_DIR/.gitmodules" ]] && git -C "$ROOT_DIR" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  submodule_path="$(
    git -C "$ROOT_DIR" config -f "$ROOT_DIR/.gitmodules" --get submodule.sherpa-onnx.path 2>/dev/null || true
  )"
  if [[ "$submodule_path" == "third_party/sherpa-onnx" ]]; then
    is_submodule=true
  fi
fi

if [[ "$is_submodule" == true ]]; then
  echo "Updating the sherpa-onnx git submodule."
  if ! git -C "$ROOT_DIR" submodule update --init --recursive -- "$submodule_path"; then
    echo "Failed to update the sherpa-onnx submodule." >&2
    exit 1
  fi
else
  if [[ ! -d "$SHERPA_REPO_DIR" ]]; then
    echo "Cloning sherpa-onnx into third_party/."
    git clone "$SHERPA_ONNX_URL" "$SHERPA_REPO_DIR"
  fi
fi

if ! git -C "$SHERPA_REPO_DIR" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "third_party/sherpa-onnx exists but is not a git repository." >&2
  exit 1
fi

if [[ "$is_submodule" == false ]]; then
  echo "Ensuring sherpa-onnx is available at commit $SHERPA_ONNX_COMMIT."
  if ! (cd "$SHERPA_REPO_DIR" && git rev-parse --verify "$SHERPA_ONNX_COMMIT^{commit}" >/dev/null 2>&1); then
    echo "Fetching sherpa-onnx commit $SHERPA_ONNX_COMMIT."
    if ! (cd "$SHERPA_REPO_DIR" && git fetch --depth 1 origin "$SHERPA_ONNX_COMMIT"); then
      echo "Failed to fetch sherpa-onnx commit $SHERPA_ONNX_COMMIT. Continuing with the existing checkout." >&2
    fi
  fi

  if ! (cd "$SHERPA_REPO_DIR" && git checkout "$SHERPA_ONNX_COMMIT"); then
    echo "Failed to checkout sherpa-onnx commit $SHERPA_ONNX_COMMIT. Continuing with the existing checkout." >&2
  fi
fi

if [[ -f "$SHERPA_C_API_DYLIB" && -f "$ORT_DYLIB" ]]; then
  echo "Native libraries already exist in third_party/sherpa-onnx-prefix/."
else
  echo "Building sherpa-onnx with a minimal C API configuration."

  BUILD_DIR="$THIRD_PARTY_DIR/sherpa-onnx-build-macos-min"
  CMAKE_GENERATOR_ARGS=()
  if command -v ninja >/dev/null 2>&1; then
    CMAKE_GENERATOR_ARGS=(-GNinja)
  fi

  cmake -S "$SHERPA_REPO_DIR" -B "$BUILD_DIR" "${CMAKE_GENERATOR_ARGS[@]}" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="$SHERPA_PREFIX_DIR" \
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

  cmake --build "$BUILD_DIR" --config Release
  cmake --install "$BUILD_DIR" --config Release
fi

PC_FILE="$SHERPA_PREFIX_DIR/sherpa-onnx.pc"
if [[ -f "$PC_FILE" ]]; then
  sed -i '' 's|^prefix=.*$|prefix=${pcfiledir}|' "$PC_FILE"
fi

if [[ ! -f "$SHERPA_C_API_DYLIB" ]]; then
  echo "Expected library not found: $SHERPA_C_API_DYLIB." >&2
  exit 1
fi

if [[ ! -f "$ORT_DYLIB" ]]; then
  echo "Expected library not found: $ORT_DYLIB." >&2
  exit 1
fi

mkdir -p "$ROOT_DIR/model"

if [[ -d "$MODEL_DIR" ]]; then
  echo "Model directory already exists: $MODEL_DIR."
else
  echo "Downloading model: $MODEL_NAME."
  curl -L --fail "$MODEL_URL" -o "$MODEL_TARBALL"

  echo "Extracting model to models/."
  tar -xjf "$MODEL_TARBALL" -C "$ROOT_DIR/model"
  rm -f "$MODEL_TARBALL"
fi

if [[ ! -d "$MODEL_DIR" ]]; then
  echo "Expected model directory not found after extraction: $MODEL_DIR." >&2
  exit 1
fi

mkdir -p "$WHISPER_MODEL_DIR"

if [[ -f "$WHISPER_MODEL_PATH" ]]; then
  echo "Whisper model already exists: $WHISPER_MODEL_PATH."
else
  echo "Whisper model is missing: $WHISPER_MODEL_PATH." >&2
  echo "Provide a GGML model at that path, or set INKFLOW_WHISPER_MODEL_PATH at runtime." >&2
fi

echo "Setup complete."
