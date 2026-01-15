# Architecture

Purpose: Define system architecture, key decisions, and repository boundaries for InkFlow.

Audience: Engineers and LLMs reading the canonical system specification.

Scope: Platform targets, frameworks, speech-to-text architecture, session model, LLM rewrite, text injection, and repository layout.

## Platform targets

- Target OS: macOS 13+ for the first release.
- Future support: Windows and Linux are not implemented, but must remain structurally supported via a `platform` abstraction layer with feature-gated stubs.

## Desktop framework

- Tauri v2 provides windowing, the event bridge, and app packaging.
- Global hotkey: `tauri-plugin-global-shortcut` registered in Rust at startup.
- Window glass effects: Transparent window plus `windowEffects` and runtime `set_effects`.
  - macOS requires `macOSPrivateApi = true` in `tauri.conf.json`, which enables `tauri`'s `macos-private-api` feature.
  - Using macOS private APIs prevents App Store distribution. This project assumes non-App Store distribution.

## UI stack

- React + TypeScript.
- Vite for builds and development server.
- Tailwind CSS for styling and iteration speed.
- Radix UI for accessible primitives.
- Framer Motion for animations.
- Optional accelerator: shadcn/ui for component scaffolding.

## JavaScript tooling

- Deno 2 for running tasks and managing npm dependencies.
- Use `deno task` as the entrypoint for UI scripts (`dev`, `build`, `fmt`, `lint`).
- Use `deno install` for local dependency caching.
- Enable `"nodeModulesDir": "auto"` to support npm lifecycle scripts as needed.

## Speech-to-text architecture (summary)

- Streaming partials and endpoints: sherpa-onnx streaming ASR (Zipformer-Transducer).
- Live refinement: Whisper sliding-window decoding in parallel during dictation.
- Finalization: Whisper second-pass decoding per endpoint segment replaces provisional sherpa text.
- Single Whisper context instance per process.
- Canonical specification: `docs/spec/stt_dictation_pipeline.md`.

## Session architecture

- Session core: A single Session Actor is the sole writer of session state.
- Workers: Audio capture, sherpa streaming, whisper window, and whisper second-pass run as worker tasks and emit pipeline events back to the actor.
- Ports/adapters: UI events, window control, and platform text injection are provided via adapters to keep core logic independent of Tauri APIs.
- Contract stability: UI commands and event payloads remain unchanged; only internal wiring changes.

## LLM rewrite

- Primary API: OpenAI Responses API via rig-core (non-streaming acceptable).
- Provider implementation: rig-core OpenAI client configured with the persisted base URL.
- Configuration: `base_url`, `api_key`, `model`, `temperature`, `system_prompt`.
- Storage: Persist settings; do not echo sensitive secrets back to the frontend.

## Text injection (macOS)

- Primary strategy: Set clipboard and synthesize Cmd+V in the previously focused app.
- Fallback strategy: Per-character typing injection (requires accessibility permission).
- Permissions UX: Detect failure and guide the user to enable Accessibility permissions.

## Repository layout (expected)

```
.
├── assets/                       # Sample audio assets for testing.
├── crates/
│   ├── sherpa-onnx-sys/           # Bindgen-generated FFI for sherpa-onnx C API.
│   └── sherpa-onnx/               # Safe wrapper (dynamic loading + JSON parsing).
├── docs/
│   ├── spec/                      # System specifications and contracts.
│   └── guide/                     # Operational and development guides.
├── models/                        # ASR models (downloaded by setup).
├── scripts/
│   └── setup_macos.sh             # Builds sherpa-onnx dylibs and downloads the model.
├── third_party/
│   ├── sherpa-onnx/               # Upstream source (git submodule).
│   └── sherpa-onnx-prefix/        # CMake install prefix (generated; dylibs, headers).
├── ui/                            # React + Vite frontend (run via Deno tasks).
│   ├── deno.json
│   ├── package.json
│   ├── vite.config.ts
│   └── src/
│       ├── app/
│       │   ├── models/
│       │   ├── tauri/
│       │   └── hooks/
│       ├── components/
│       ├── styles/
│       └── main.tsx
├── src-tauri/                     # Tauri backend (Rust).
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── Info.plist
│   ├── capabilities/
│   │   └── default.json
│   ├── permissions/
│   │   └── app.toml               # App command permissions for capabilities.
│   └── src/
│       ├── application/
│       ├── audio/
│       ├── domain/
│       ├── pipeline/
│       ├── platform/
│       ├── ports/
│       ├── adapters/
│       ├── commands.rs
│       ├── events.rs
│       ├── llm.rs
│       ├── session.rs
│       ├── stt.rs
│       └── stt_trace.rs
└── Makefile.toml                  # Use `cargo make fmt/clippy/nextest`.
```

## Key boundaries

### `platform/*`

Defines all OS-specific behavior behind traits. The rest of the code must not call macOS APIs directly.

Required traits (minimum):

- `PlatformHotkey`
- `PlatformTextInjector`
- `PlatformWindowEffects`

### `domain/*`

Pure transcript and merge logic with no Tauri or OS dependencies.

### `application/*`

Owns the Session Actor and session service. It is the only layer allowed to mutate session state.

### `pipeline/*`

Runs audio capture, streaming ASR, and whisper workers. All results are sent back to the Session Actor as events.

### `ports/*` and `adapters/*`

Ports define external boundaries (UI events and platform injection). Adapters implement them using Tauri and OS APIs.

### `stt.rs`

Speech-to-text integration uses sherpa-onnx streaming ASR via the C API wrapper crates:

- `crates/sherpa-onnx-sys`: Bindgen-generated unsafe FFI.
- `crates/sherpa-onnx`: Safe wrapper and runtime dynamic loading.

### `llm.rs`

LLM rewriting uses the rig-core OpenAI Responses API with persisted settings and a configurable base URL.
