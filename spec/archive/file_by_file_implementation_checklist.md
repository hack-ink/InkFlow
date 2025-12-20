# Archive: File-by-file Implementation Checklist

This checklist is intentionally explicit. Each file includes its purpose, key types and APIs, and the implementation notes required to complete it.

Status: Historical planning document. It is retained for reference, but parts may be out of date.

If you are looking for a practical starting point, read `spec/80_work_plan.md` first.

## Backend (`src-tauri/`)

### `src-tauri/src/main.rs`

Purpose:

- Boot the Tauri app, create the overlay window, register the global hotkey, and initialize shared state.

Key tasks:

- Register `tauri-plugin-global-shortcut` in Rust.
- Create or access the `main` webview window and keep it hidden by default.
- Apply macOS window effects (glass material) at startup for the overlay window.
- On hotkey press (`Alt+Space`): dispatch `SessionAction::StartNew`.

Notes:

- On macOS, transparent windows require `macOSPrivateApi = true` in `tauri.conf.json`.
- Avoid complex global state; use `tauri::State<AppState>`.

### `src-tauri/src/app_state.rs`

Purpose:

- Central container for the app: session manager, settings store, and platform adapters.

Key types:

- `AppState`
- `SessionManager`
- `SettingsStore`
- `Platform` trait object

### `src-tauri/src/error.rs`

Purpose:

- One error type for commands and events.

Key tasks:

- Define `AppErrorCode` (stringly stable) and `AppError { code, message }`.
- Add `recoverable: bool` to allow UI to decide whether to keep the overlay open.
- Ensure errors are safe to expose to the UI (no secrets).

### `src-tauri/src/events.rs`

Purpose:

- Canonical event names and strongly typed payloads.

Key tasks:

- Define event constants:
  - `SESSION_STATE`
  - `STT_PARTIAL`
  - `STT_FINAL`
  - `LLM_REWRITE`
  - `ERROR`
- Implement small helper functions:
  - `emit_session_state(app, payload)`
  - `emit_stt_partial(app, payload)`
  - `emit_error(app, payload)`

### `src-tauri/src/commands.rs`

Purpose:

- Tauri commands exposed to the UI.

Commands:

- `session_dispatch(action: SessionAction) -> SessionSnapshot`
- `settings_get() -> Settings`
- `settings_update(patch: SettingsPatch) -> Settings`
- `permissions_request_microphone() -> bool`
- `platform_open_system_settings(target: SystemSettingsTarget) -> ()`

Rules:

- Do not return secrets (e.g., API key).

### `src-tauri/src/session.rs`

Purpose:

- Session orchestration, task cancellation, and high-level event emission.

Key tasks:

- Maintain a single active session.
- Own a cancellation mechanism to stop microphone and background tasks when the session ends.
- Emit `session/state` on every transition.

### `src-tauri/src/session/state_machine.rs`

Purpose:

- Define the allowed state transitions.

Key types:

- `SessionState`
- `SessionAction`
- `SessionSnapshot`

Required behavior:

- `Alt+Space` starts a new session even if one is active (cancel and restart).
- `Enter`:
  - Listening -> Finalizing -> Rewriting -> RewriteReady
  - RewriteReady -> Injecting -> Hidden
- `Esc`: cancel and return to Hidden from any state.
- Silence timeout: Listening -> Finalizing automatically.

### `src-tauri/src/platform.rs`

Purpose:

- Cross-platform boundary.

Required traits:

- `PlatformHotkey`
- `PlatformTextInjector`
- `PlatformWindowEffects`

Implementation notes:

- The macOS implementation is the only real one initially.
- Windows/Linux files must compile as stubs and return `UnsupportedPlatform` errors.

### `src-tauri/src/platform/macos.rs`

Purpose:

- macOS implementation: hotkey integration, window effects, and text injection.

Key tasks:

- Implement injection via clipboard + `Cmd+V` event.
- Detect injection failures and emit an actionable `error` event guiding the user to enable Accessibility permissions.
- Provide `open_system_settings(target)` mapping to the relevant preference panes.

### `src-tauri/src/platform/windows.rs` and `src-tauri/src/platform/linux.rs`

Purpose:

- Stubs for future support.

Key tasks:

- Compile with feature gates.
- Return `UnsupportedPlatform` errors with clear messages.

### `src-tauri/src/audio.rs`

Purpose:

- Microphone input, VAD chunking, and silence timeout.

Key tasks:

- Request microphone permission on session start (or show a permission UI state).
- Build an audio stream abstraction consumed by STT engines.
- Implement VAD chunking pipeline as the primary pseudo-streaming mechanism.

### `src-tauri/src/stt.rs`

Purpose:

- STT integration and configuration for the app.

Key tasks:

- Load a sherpa-onnx online recognizer via the `crates/sherpa-onnx` wrapper.
- Resolve model paths from environment variables with executable-relative defaults.
- Fail fast with actionable error messages when model files or runtime libraries are missing.

### `crates/sherpa-onnx-sys/*`

Purpose:

- bindgen-generated unsafe FFI bindings for the sherpa-onnx C API (online/streaming subset).

Key tasks:

- Keep the vendored minimal header in sync with upstream sherpa-onnx C API when upgrading.
- Keep bindings generation deterministic and minimal (online APIs only).

### `crates/sherpa-onnx/*`

Purpose:

- Safe wrapper around `sherpa-onnx-sys` with RAII and JSON result parsing.

Key tasks:

- Load `libsherpa-onnx-c-api` dynamically at runtime.
- Provide `OnlineRecognizer` + `OnlineStream` APIs for streaming ASR.

### `src-tauri/src/llm.rs`

Purpose:

- Provider abstraction for rewriting text.

Key tasks:

- Define `LlmProvider` trait.
- Provide OpenAI-compatible provider implementation.

### `src-tauri/src/llm/openai.rs`

Purpose:

- OpenAI Chat Completions style rewrite call.

Key tasks:

- Build request: system prompt + user text.
- Handle errors and timeouts.
- Return rewritten text only (no streaming required for the first version).

### `src-tauri/src/settings/schema.rs`

Purpose:

- Settings structure.

Minimum fields:

- LLM: `base_url`, `api_key` (stored securely), `model`, `temperature`, `system_prompt`
- STT: `strategy`, `silence_timeout_ms`, sliding-window config
- Injection: mode selection, clipboard restore behavior

### `src-tauri/src/settings/store.rs`

Purpose:

- Persist settings and handle forward-compatible upgrades.

Key tasks:

- Store a settings schema version.
- Support migration defaults when new fields are added.
- Store secrets in a secure storage solution (macOS keychain) or in a separate protected file.

### `src-tauri/src/inject.rs`

Purpose:

- Orchestrate injection and related UI transitions.

Key tasks:

- Emit `session/state = Injecting` before injection begins.
- Attempt paste-based injection first, then fallback to type-based injection if enabled.

## Frontend (`ui/`)

### `ui/src/app/tauri/events.ts`

Purpose:

- Event subscriptions and typed handlers.

Key tasks:

- Subscribe to all backend events.
- Ensure partial updates are applied only when `revision` is newer.

### `ui/src/app/tauri/commands.ts`

Purpose:

- Typed `invoke()` wrappers.

Key tasks:

- Implement `sessionDispatch(action)`, `settingsGet()`, `settingsUpdate(patch)`.
- Normalize error handling for UI.

### `ui/src/app/hooks/useSessionEvents.ts`

Purpose:

- Convert backend events into React state.

Key tasks:

- Maintain `state`, `rawText`, `rewriteText`, `error`.
- Prevent flicker by applying partial updates only when revision increases.

### `ui/src/components/OverlayShell.tsx`

Purpose:

- Glassmorphism container (noise + highlights + blur layers).

Key tasks:

- Define CSS layers: background tint, border highlight, subtle noise, gradient sheen.

### `ui/src/components/RecorderPulse.tsx`

Purpose:

- Listening-state pulse animation.

Key tasks:

- Use Framer Motion for smooth start/stop transitions.
- Ensure animation is paused when not listening to save resources.

### `ui/src/components/TranscriptView.tsx`

Purpose:

- Render partial transcript updates smoothly.

Key tasks:

- Render full text first.
- Optional enhancement: diff-based render for only appended segments.

### `ui/src/components/RewritePane.tsx`

Purpose:

- Show rewriting progress and allow editing before injection.

Key tasks:

- Provide editable text area in `RewriteReady`.
- Second Enter injects the edited version.

### `ui/src/components/SettingsDrawer.tsx`

Purpose:

- Configure LLM, STT, and injection settings.

Key tasks:

- API key input must not display existing secret; only show “configured” status.

### `ui/src/components/PermissionGate.tsx`

Purpose:

- Permission error UI and system settings shortcuts.

Key tasks:

- Microphone denied: guide user to Privacy settings.
- Accessibility denied: guide user to enable it for injection.
