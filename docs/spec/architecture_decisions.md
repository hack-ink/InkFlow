# Architecture Decisions

This document records the choices that shape the implementation plan.

## Platform and Runtime

- **Target OS:** macOS 13+ only for the first release.
- **Future support:** Windows/Linux are not implemented but must be structurally supported via a `platform` abstraction layer with feature-gated stubs.

## Desktop Framework

- **Tauri v2** for windowing, event bridge, and app packaging.
- **Global hotkey:** `tauri-plugin-global-shortcut` registered in Rust at startup.
- **Window glass effects:** Use transparent window + `windowEffects` and/or runtime `set_effects`.
  - macOS requires `macOSPrivateApi = true` in `tauri.conf.json`, which enables `tauri`’s `macos-private-api` feature.
  - Using macOS private APIs prevents App Store distribution. This project assumes non–App Store distribution.

## UI Stack (Mature, Popular, Maintained)

- **React + TypeScript** for a stable ecosystem and broad maintainer support.
- **Vite** for fast dev server and reliable bundling for Tauri.
- **Tailwind CSS** for rapid glassmorphism iteration and consistent styling.
- **Radix UI** (primitives) for accessible components without forcing a visual design system.
- **Framer Motion** for high-quality, composable animations with predictable performance.
- **Optional accelerator:** shadcn/ui (code generator) to bootstrap common components while keeping Radix + Tailwind as the underlying foundation.

## JavaScript Tooling

- **Deno 2** for running tasks and managing npm dependencies.
  - Use `deno install` (local installation) to add and cache npm packages.
  - Enable `"nodeModulesDir": "auto"` to support npm lifecycle scripts when needed by tooling.
  - Use `deno task` as the single entrypoint for UI scripts (`dev`, `build`, `fmt`, `lint`).

## Speech-to-Text (Two-pass: sherpa streaming + whisper)

- **Streaming partials and endpoints:** sherpa-onnx streaming ASR (Zipformer-Transducer) via the sherpa-onnx C API.
  - Goal: low-latency, local, streaming partials with endpoint-based segment boundaries.
- **Live refinement (planned):** run Whisper in a rolling window in parallel to continuously improve the on-screen live transcript while speaking.
  - Canonical spec: `docs/spec/stt_dictation_pipeline_spec.md`.
- **Finalization:** on each endpoint, run Whisper on that segment’s audio and replace the provisional sherpa segment text.
- **Integration:** A thin Rust wrapper around the C API.
  - `crates/sherpa-onnx-sys`: bindgen-generated unsafe FFI for a minimal online C API surface.
  - `crates/sherpa-onnx`: safe RAII wrapper + JSON result parsing.
  - Runtime library loading uses `libloading` to avoid hard-linking at build time.
- **Whisper integration:** `whisper-rs` (whisper.cpp) for sliding-window refinement and per-endpoint second pass.
  - Use a single Whisper model instance per process; do not load multiple copies.
- **Default model:** `sherpa-onnx-streaming-zipformer-en-2023-06-21` (English, streaming).
- **Default quantization:** int8 encoder + int8 joiner + fp32 decoder (with an opt-in int8 decoder).
- **Endpointing defaults:** enabled with rule1=2.4s, rule2=1.2s, rule3=300.0s (tunable).

Rationale:

- sherpa-onnx provides the lowest-latency streaming feedback, but it is typically less accurate than Whisper.
- Whisper-only streaming feels delayed at small step sizes; using it as a second pass (and optionally as a sliding-window refinement) improves quality while keeping low-latency behavior.

## Session Architecture (Actor + Ports/Adapters)

- **Session core:** A single Session Actor is the sole writer of session state.
- **Workers:** Audio capture, sherpa streaming, whisper window, and whisper second-pass run as worker tasks and emit pipeline events back to the actor.
- **Ports/adapters:** UI event emission, window control, and platform text injection are provided via adapters to keep core logic independent of Tauri APIs.
- **Contract stability:** UI commands and event payloads remain unchanged; only internal wiring changes.

## LLM Rewrite

- **Primary API:** OpenAI Responses API via rig-core (non-streaming is acceptable).
- **Provider implementation:** rig-core OpenAI client configured with the persisted base URL.
- **Configuration:** `base_url`, `api_key`, `model`, `temperature`, `system_prompt`.
- **Storage:** Persist settings; do not echo sensitive secrets back to the frontend.

## Text Injection (macOS)

- **Primary strategy:** clipboard set + synthetic `Cmd+V` in the previously focused app.
- **Fallback strategy:** per-character typing injection (requires accessibility permission).
- **Permissions UX:** Detect failure and guide the user to enable Accessibility permissions.
