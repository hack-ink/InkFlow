# Backend and UI Contract (Events and Commands)

This document defines the event names, payload schemas, and command signatures that connect the Rust backend to the UI.

The backend is the single source of truth for session state. The UI should not implement its own session transitions beyond sending user intents.

## Event Names (Backend -> Frontend)

All events include a `session_id` when a session exists.

- `session/state`
- `stt/partial`
- `stt/final`
- `llm/rewrite`
- `error`

### `session/state`

Payload:

```ts
export type SessionState =
  | "Hidden"
  | "Showing"
  | "Listening"
  | "Finalizing"
  | "Rewriting"
  | "RewriteReady"
  | "Injecting"
  | "Error";

export type SessionStateEvent = {
  session_id: string;
  state: SessionState;
  reason?: string;
};
```

### `stt/partial`

Payload:

```ts
export type SttStrategy = "vad_chunk" | "sliding_window";

export type SttPartialEvent = {
  session_id: string;
  text: string;
  revision: number;
  strategy: SttStrategy;
};
```

### `stt/final`

Payload:

```ts
export type SttFinalEvent = {
  session_id: string;
  text: string;
};
```

### `llm/rewrite`

Payload:

```ts
export type LlmRewriteEvent = {
  session_id: string;
  text: string;
  model: string;
};
```

### `error`

Payload:

```ts
export type ErrorEvent = {
  session_id?: string;
  code: string;
  message: string;
  recoverable: boolean;
};
```

## Commands (Frontend -> Backend)

The UI sends intent, the backend executes and emits state updates.

### `overlay_set_height`

Resize the overlay window to a specific logical height.

Payload:

```ts
export type OverlaySetHeightArgs = {
  height: number;
  animate: boolean;
};
```

Rust signature skeleton:

```rust
#[tauri::command]
async fn overlay_set_height(
  app: tauri::AppHandle,
  height: f64,
  animate: bool,
) -> Result<(), AppError> {
  // Implementation is platform-specific.
  Ok(())
}
```

Notes:

- The UI should measure its expanded content and request an exact window height.
- On macOS, prefer native window animations for smooth resizing.

### `session_dispatch`

Intent payload:

```ts
export type SessionAction =
  | { type: "show" }
  | { type: "start_new" }
  | { type: "enter" }
  | { type: "escape" }
  | { type: "rewrite" };
```

Return payload:

```ts
export type SessionSnapshot = {
  session_id?: string;
  state: SessionState;
  raw_text: string;
  rewrite_text?: string;
};
```

Rust signature skeleton:

```rust
#[tauri::command]
async fn session_dispatch(
  app: tauri::AppHandle,
  state: tauri::State<'_, AppState>,
  action: SessionAction,
) -> Result<SessionSnapshot, AppError> {
  state.session().dispatch(&app, action).await
}
```

### Settings

`settings_get() -> Settings`

`settings_update(patch: SettingsPatch) -> Settings`

Rules:

- Never return the API key plaintext back to the frontend.
- Expose a boolean like `llm.has_api_key` for UI state.

### Permissions / System Settings (macOS only)

`platform_open_system_settings(target: "microphone" | "accessibility" | "input_monitoring") -> ()`

## Hotkey (Option+Space)

Hotkey handling is on the backend. When triggered:

1. If the overlay is already visible, hide it by dispatching `SessionAction::Escape`.
2. Otherwise, show the overlay window and dispatch `SessionAction::Show`.
3. The UI handles Space press/release to dispatch `SessionAction::StartNew` and `SessionAction::Enter`.

The overlay window should hide when it loses focus (Spotlight-like behavior).

The UI should not be responsible for global hotkey registration.
