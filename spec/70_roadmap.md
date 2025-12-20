# Roadmap

Each milestone is independently verifiable. The goal is to have a usable product early, then iterate.

## Milestone 0: Repository Rebase

Deliverables:

- Replace the template repository structure with `ui/` + `src-tauri/`.
- Keep `Makefile.toml` for `cargo make fmt/clippy/nextest`.
- Ensure `cargo make fmt` runs successfully on the backend crate.

Suggested changes (explicit):

- Delete template code:
  - `src/`
  - Root `build.rs` (if not required by the new workspace)
  - Root `Cargo.toml` and `Cargo.lock` (replace with a workspace layout)
- Create new structure:
  - `ui/` (React + Vite)
  - `src-tauri/` (Tauri v2 backend)
  - `src-tauri/capabilities/default.json`
  - `src-tauri/Info.plist`
  - `src-tauri/tauri.conf.json`

Acceptance:

- `deno task --cwd ui dev` starts the dev server.
- `cargo tauri dev` runs the desktop app and loads the UI.

## Milestone 1: Overlay Window + Global Hotkey

Deliverables:

- Overlay window exists, hidden by default, shown on `Option(Alt)+Space`.
- Glass effect enabled (transparent + window effects).

Acceptance:

- Hotkey toggles the overlay reliably.
- No visible white flash on show/hide.

## Milestone 2: Session State Machine Skeleton

Deliverables:

- Implement `SessionState` and `SessionAction`.
- Emit `session/state` on every state change.
- UI renders based on state and supports `Enter` and `Esc`.

Acceptance:

- Press `Esc` cancels from any state.
- Press `Enter` transitions along the expected path even with placeholder implementations.

## Milestone 3: Microphone Permissions + Audio Capture

Deliverables:

- Request microphone permission when session starts.
- If permission is denied, show actionable UI guidance.
- Start and stop audio capture in `Listening`.

Acceptance:

- Denying microphone permission produces a clear error and does not crash.
- Granting permission starts capture and shows the Listening UI.

## Milestone 4: STT Pseudo-Streaming (Primary Strategy)

Deliverables:

- Implement the sherpa-onnx streaming STT engine (Zipformer-Transducer) via the C API wrapper.
- Emit `stt/partial` updates continuously from the online stream.
- On first `Enter`, stop capture, flush the online stream, and emit `stt/final`.

Acceptance:

- Speaking updates the UI within an acceptable latency budget.
- Final text replaces partial text on first `Enter`.

## Milestone 5: Sliding-Window Fallback (Optional but Recommended)

Deliverables:

- Implement periodic re-transcription and diff merge.
- Configuration switch: select STT strategy.

Acceptance:

- With fallback enabled, the transcript still updates smoothly without frequent full text resets.

## Milestone 6: LLM Rewrite

Deliverables:

- OpenAI-compatible rewrite provider.
- Store and edit LLM settings.
- On first `Enter`, automatically rewrite and transition to `RewriteReady`.

Acceptance:

- Missing API key is handled gracefully with an actionable error.
- Successful rewrite produces `llm/rewrite` and updates UI.

## Milestone 7: Text Injection + Permissions UX

Deliverables:

- On second `Enter`, inject text into the previously focused app.
- Primary injection: clipboard + `Cmd+V`.
- Fallback: typing injection (optional).
- Accessibility permission guidance on failure.

Acceptance:

- Text is injected into a standard macOS text field (e.g., Notes) reliably.
- Overlay closes after injection.

## Milestone 8: Polish Pass

Deliverables:

- Smooth animations for open/close, pulse, and text updates.
- Edge-case handling: rapid hotkey presses, quick cancel, network failure during rewrite.
- Optional: model download/loading UX.

Acceptance:

- The app remains responsive under repeated use.
- UI feels stable, smooth, and professional.

## Dependency Audit (Build vs Buy)

Notes:

- LLM rewriting now uses rig-core and the OpenAI Responses API.
- Audio capture and text injection remain custom because the current requirements depend on macOS-specific behavior and reliability.
- Evaluate secure secret storage (`keyring`) and layered configuration (`config`) if settings grow beyond the current scope.
