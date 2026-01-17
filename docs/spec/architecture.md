# Architecture

Purpose: Define system architecture, key decisions, and repository boundaries for InkFlow.

Audience: Engineers and LLMs reading the canonical system specification.

Scope: Platform targets, frameworks, speech-to-text architecture, session model, text injection, and repository layout.

## Platform targets

- Target OS: macOS 26+ for the first release.
- Future support: Windows and Linux are not implemented, but the core Rust engine must remain platform-agnostic.

## Desktop framework

- SwiftUI provides the macOS user interface.
- Liquid Glass is the primary visual system for macOS 26.
- AVFoundation provides native audio capture.
- The macOS app communicates with the Rust engine through a C ABI FFI layer.

## Rust core

- `crates/inkflow-core` owns the speech pipeline, decoding, and merge logic.
- `crates/inkflow-ffi` exposes a stable C ABI and delivers updates via callbacks.
- The SwiftUI app must treat the Rust engine as the source of truth for transcript updates.

## Speech-to-text architecture (summary)

- Streaming partials and endpoints: sherpa-onnx streaming ASR (Zipformer-Transducer).
- Live refinement: Whisper sliding-window decoding in parallel during dictation.
- Finalization: Whisper second-pass decoding per endpoint segment replaces provisional sherpa text.
- Single Whisper context instance per process.
- Canonical specification: `docs/spec/core/stt_dictation_pipeline.md`.

## Text injection (macOS)

- Primary strategy: Set clipboard and synthesize Cmd+V in the previously focused app.
- Fallback strategy: Per-character typing injection (requires accessibility permission).
- Permissions UX: Detect failure and guide the user to enable Accessibility permissions.

## Settings UI (macOS)

- Entry points: the menubar status item menu includes Settings... (Command-,) and Quit (Command-Q).
- The settings window uses a sidebar with three sections: Appearance, Microphone, Shortcuts.
- Appearance settings apply immediately to the app UI.
- The Microphone section supports input device selection and a non-recording input test with a live level meter.
- The Shortcuts section is interactive UI only and does not persist or affect behavior in this version.

## Floating panel (macOS)

- The primary UI is a borderless NSPanel styled as a Spotlight-like floating surface.
- The panel hides when it loses key focus and can be reopened from the menubar, dock, or shortcuts.
- The panel is repositionable by dragging empty space, and the last position is restored on the next show.

## Repository layout (expected)

```
.
├── apps/
│   └── macos/
│       └── InkFlow/                 # SwiftUI macOS app (Xcode project).
├── crates/
│   ├── inkflow-core/                # Rust core engine and STT pipeline.
│   ├── inkflow-ffi/                 # C ABI wrapper around inkflow-core.
│   ├── sherpa-onnx-sys/             # Bindgen-generated FFI for sherpa-onnx C API.
│   └── sherpa-onnx/                 # Safe wrapper (dynamic loading + JSON parsing).
├── docs/
│   ├── spec/                        # System specifications and contracts.
│   └── guide/                       # Operational and development guides.
├── models/                          # ASR models (downloaded by setup).
├── scripts/                         # Setup and build helpers.
├── third_party/                     # Upstream and build outputs for sherpa-onnx.
└── Makefile.toml                    # Use `cargo make fmt`, `cargo make lint`, and `cargo make nextest`.
```

## Key boundaries

### `crates/inkflow-core`

Owns speech recognition, decoding, and merge logic. It must remain platform-agnostic.

### `crates/inkflow-ffi`

Exposes a stable C ABI for Swift. The ABI is the only supported integration point for the macOS app.

### `apps/macos/InkFlow`

SwiftUI app that captures audio, forwards frames to the Rust engine, and renders transcript updates.

### `domain/*` (inside `inkflow-core`)

Pure transcript and merge logic with no platform dependencies.

### `stt/*` (inside `inkflow-core`)

Speech-to-text integration using sherpa-onnx streaming ASR and whisper-rs.
