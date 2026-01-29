#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="${ROOT_DIR}/apps/macos/InkFlow"
PROJECT="${APP_DIR}/InkFlow.xcodeproj"
SCHEME="InkFlow"
DERIVED_DATA_DIR="${APP_DIR}/.build"
RENDER_DEBUG_LOG="${RENDER_DEBUG_LOG:-inkflow_core::engine::render=debug,inkflow_core=info}"

MODE="${1:-}"
if [ -n "${MODE}" ]; then
  case "${MODE}" in
    debug|Debug|DEBUG)
      CONFIGURATION="Debug"
      ;;
    release|Release|RELEASE)
      CONFIGURATION="Release"
      ;;
    *)
      echo "Unknown configuration \"${MODE}\". Use \"debug\" or \"release\"." >&2
      exit 1
      ;;
  esac
elif [ -n "${CONFIGURATION:-}" ]; then
  CONFIGURATION="${CONFIGURATION}"
else
  CONFIGURATION="Debug"
fi

if [ -d "${HOME}/.cargo/bin" ]; then
  export PATH="${HOME}/.cargo/bin:${PATH}"
fi

if ! command -v xcodebuild >/dev/null 2>&1; then
  echo "xcodebuild is not available on PATH." >&2
  exit 1
fi

if command -v xcbeautify >/dev/null 2>&1; then
  xcodebuild \
    -project "${PROJECT}" \
    -scheme "${SCHEME}" \
    -configuration "${CONFIGURATION}" \
    -destination "platform=macOS" \
    -derivedDataPath "${DERIVED_DATA_DIR}" \
    build 2>&1 | xcbeautify --quiet
  BUILD_STATUS=${PIPESTATUS[0]}
else
  xcodebuild \
    -project "${PROJECT}" \
    -scheme "${SCHEME}" \
    -configuration "${CONFIGURATION}" \
    -destination "platform=macOS" \
    -derivedDataPath "${DERIVED_DATA_DIR}" \
    build
  BUILD_STATUS=$?
fi

if [ "${BUILD_STATUS}" -ne 0 ]; then
  exit "${BUILD_STATUS}"
fi

APP_PATH="${DERIVED_DATA_DIR}/Build/Products/${CONFIGURATION}/Ink Flow.app"
if [ ! -d "${APP_PATH}" ]; then
  echo "InkFlow.app not found at ${APP_PATH}." >&2
  exit 1
fi

APP_BINARY="${APP_PATH}/Contents/MacOS/InkFlow"
if [ -x "${APP_BINARY}" ]; then
  export RUST_LOG="${RUST_LOG:-${RENDER_DEBUG_LOG}}"
  "${APP_BINARY}" &
  exit 0
fi

export RUST_LOG="${RUST_LOG:-${RENDER_DEBUG_LOG}}"
open "${APP_PATH}"
