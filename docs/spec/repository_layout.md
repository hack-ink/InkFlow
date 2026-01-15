# Repository Layout

This is the proposed repository tree and the minimal boundaries that keep the app macOS-first but cross-platform-ready.

The current repository is a template and may be replaced. The layout below is the intended “end state” for the first working version.

## Template Cleanup (Current Repo)

When rebasing from this template, remove anything that is not needed for a Tauri v2 desktop app. Typical deletions include:

- `src/` (CLI entrypoints).
- Root `build.rs` (unless still needed after switching to Tauri).
- Root `Cargo.toml` and `Cargo.lock` (replace with a workspace `Cargo.toml` and a new lockfile).

Files typically kept:

- `Makefile.toml` (repository rule: use `cargo make fmt/clippy/nextest`).
- `rust-toolchain.toml` (toolchain pin).
- `AGENTS.md` and `STYLE_RUST.md` (repository rules and conventions).

## Proposed Tree

```
.
├── crates/
│   ├── sherpa-onnx-sys/           # bindgen-generated FFI for sherpa-onnx C API.
│   └── sherpa-onnx/               # Safe wrapper (dynamic loading + JSON parsing).
├── docs/spec/                          # Architecture, implementation notes, and context.
│   └── context/                   # Rolling 7-day context digests.
├── model/                         # ASR models (downloaded by setup).
├── script/
│   └── setup_macos.sh             # Builds sherpa-onnx dylibs and downloads the model.
├── third_party/
│   ├── sherpa-onnx/               # Upstream source (git submodule).
│   └── sherpa-onnx-prefix/        # CMake install prefix (generated; dylibs, headers).
├── ui/                           # React + Vite frontend (run via Deno tasks).
│   ├── deno.json
│   ├── package.json
│   ├── vite.config.ts
│   └── src/
│       ├── app/
│       │   ├── model/
│       │   ├── tauri/
│       │   └── hooks/
│       ├── components/
│       ├── styles/
│       └── main.tsx
├── src-tauri/                    # Tauri backend (Rust).
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── Info.plist
│   ├── capabilities/
│   │   └── default.json
│   ├── permissions/
│   │   └── app.toml              # App command permissions for capabilities.
│   └── src/
│       ├── app_state.rs
│       ├── audio.rs
│       ├── audio/
│       │   └── mic_stream.rs
│       ├── adapters.rs
│       ├── adapters/
│       │   ├── platform.rs
│       │   └── ui.rs
│       ├── application.rs
│       ├── application/
│       │   ├── session_actor.rs
│       │   └── session_service.rs
│       ├── commands.rs
│       ├── domain.rs
│       ├── domain/
│       │   └── transcript.rs
│       ├── engine.rs
│       ├── error.rs
│       ├── events.rs
│       ├── lib.rs
│       ├── llm.rs
│       ├── main.rs
│       ├── overlay.rs
│       ├── pipeline.rs
│       ├── pipeline/
│       │   ├── dictation.rs
│       │   └── types.rs
│       ├── platform.rs
│       ├── platform/
│       │   ├── macos.rs
│       │   ├── windows.rs         # Stub (feature-gated).
│       │   └── linux.rs           # Stub (feature-gated).
│       ├── ports.rs
│       ├── ports/
│       │   ├── platform.rs
│       │   └── ui.rs
│       ├── session.rs
│       ├── settings.rs
│       ├── stt.rs
│       ├── stt/
│       │   └── whisper.rs
│       └── stt_trace.rs
└── Makefile.toml                  # Use `cargo make fmt/clippy/nextest`.
```

## Key Boundaries

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

- `crates/sherpa-onnx-sys`: bindgen-generated unsafe FFI.
- `crates/sherpa-onnx`: safe wrapper and runtime dynamic loading.

### `llm.rs`

LLM rewriting uses the rig-core OpenAI Responses API with persisted settings and a configurable base URL.
