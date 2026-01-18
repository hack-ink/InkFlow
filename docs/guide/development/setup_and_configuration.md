# Setup and Configuration

This document captures the SwiftUI + Rust FFI configuration used by InkFlow.

## Xcode Project (apps/macos/InkFlow)

Key build settings:

- Bundle identifier: `ink.hack.InkFlow`.
- Bridging header: `InkFlow/InkFlowBridge.h`.
- Header search path: `$(SRCROOT)/../../../packages/inkflow-ffi/include`.
- Run Script phase: build `inkflow-ffi`, copy `libinkflow_ffi.dylib`, stage sherpa-onnx and ONNX Runtime dylibs, and link `models/` into app resources for development.

### Run Script Phase (Build inkflow-ffi)

This is the canonical script used in the Xcode build phase.

```sh
set -euo pipefail
ROOT_DIR=${SRCROOT}/../../..
CONFIG=${CONFIGURATION}
TARGET_DIR=${ROOT_DIR}/target

if command -v /bin/zsh >/dev/null 2>&1; then
  eval "$(/bin/zsh -lc 'printf "export PATH=%q\n" "$PATH"')"
fi
if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is not available in the Xcode build environment." >&2
  echo "Install Rust or ensure $HOME/.cargo/bin is on PATH." >&2
  exit 1
fi

if [ "${CONFIG}" = "Release" ]; then
  cargo build -p inkflow-ffi --release --manifest-path "${ROOT_DIR}/Cargo.toml"
  LIB_PATH=${TARGET_DIR}/release/libinkflow_ffi.dylib
  DEPS_LIB_PATH=${TARGET_DIR}/release/deps/libinkflow_ffi.dylib
else
  cargo build -p inkflow-ffi --manifest-path "${ROOT_DIR}/Cargo.toml"
  LIB_PATH=${TARGET_DIR}/debug/libinkflow_ffi.dylib
  DEPS_LIB_PATH=${TARGET_DIR}/debug/deps/libinkflow_ffi.dylib
fi

if [ ! -f "${LIB_PATH}" ]; then
  echo "InkFlow FFI library not found at ${LIB_PATH}." >&2
  exit 1
fi

install_name_tool -id @rpath/libinkflow_ffi.dylib "${LIB_PATH}"
if [ -f "${DEPS_LIB_PATH}" ]; then
  install_name_tool -id @rpath/libinkflow_ffi.dylib "${DEPS_LIB_PATH}"
fi

cp -f "${LIB_PATH}" "${BUILT_PRODUCTS_DIR}/libinkflow_ffi.dylib"
mkdir -p "${TARGET_BUILD_DIR}/${CONTENTS_FOLDER_PATH}/Frameworks"
cp -f "${LIB_PATH}" "${TARGET_BUILD_DIR}/${CONTENTS_FOLDER_PATH}/Frameworks/libinkflow_ffi.dylib"

SHERPA_DYLIB_SOURCE="${ROOT_DIR}/third_party/sherpa-onnx-prefix/lib/libsherpa-onnx-c-api.dylib"
SHERPA_DYLIB_DEST="${TARGET_BUILD_DIR}/${CONTENTS_FOLDER_PATH}/Frameworks/libsherpa-onnx-c-api.dylib"
if [ ! -f "${SHERPA_DYLIB_SOURCE}" ]; then
  echo "Sherpa-onnx C API library not found at ${SHERPA_DYLIB_SOURCE}." >&2
  echo "Run cargo make setup-macos to build the native libraries." >&2
  exit 1
fi
cp -f "${SHERPA_DYLIB_SOURCE}" "${SHERPA_DYLIB_DEST}"

ONNX_DYLIB_SOURCE="${ROOT_DIR}/third_party/sherpa-onnx-prefix/lib/libonnxruntime.1.17.1.dylib"
ONNX_DYLIB_DEST="${TARGET_BUILD_DIR}/${CONTENTS_FOLDER_PATH}/Frameworks/libonnxruntime.1.17.1.dylib"
if [ ! -f "${ONNX_DYLIB_SOURCE}" ]; then
  echo "ONNX Runtime library not found at ${ONNX_DYLIB_SOURCE}." >&2
  echo "Run cargo make setup-macos to build the native libraries." >&2
  exit 1
fi
cp -f "${ONNX_DYLIB_SOURCE}" "${ONNX_DYLIB_DEST}"
ln -sf libonnxruntime.1.17.1.dylib "${TARGET_BUILD_DIR}/${CONTENTS_FOLDER_PATH}/Frameworks/libonnxruntime.dylib"

MODELS_SOURCE="${ROOT_DIR}/models"
MODELS_DEST="${TARGET_BUILD_DIR}/${CONTENTS_FOLDER_PATH}/Resources/models"
if [ -d "${MODELS_SOURCE}" ]; then
  rm -rf "${MODELS_DEST}"
  ln -s "${MODELS_SOURCE}" "${MODELS_DEST}"
fi
```

## Info.plist (Microphone Usage)

Configure the microphone usage string via the Xcode build setting:

- `INFOPLIST_KEY_NSMicrophoneUsageDescription = "Ink Flow needs microphone access to transcribe your speech into text."`

## Code Signing Entitlements

The macOS app uses separate entitlements for Debug and Release:

- `apps/macos/InkFlow/InkFlow/InkFlow.Debug.entitlements` (Debug)
- `apps/macos/InkFlow/InkFlow/InkFlow.entitlements` (Release)

Debug disables the app sandbox so the app can read local `models/` assets via the symlinked
Resources directory. Release keeps the app sandbox enabled.

Both Debug and Release entitlements must include `com.apple.security.device.audio-input` to
allow microphone access when Hardened Runtime is enabled.

## FFI Headers

- The C ABI header lives at `packages/inkflow-ffi/include/inkflow.h`.
- The Swift bridging header lives at `apps/macos/InkFlow/InkFlow/InkFlowBridge.h` and includes `inkflow.h`.

## Cargo Make (Repository Rules)

Use these tasks for formatting and validation:

- `cargo make fmt`
- `cargo make lint`
- `cargo make nextest`
