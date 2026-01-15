# Architecture Upgrade Plan (Actor + Ports/Adapters)

This plan captures the task breakdown for the session architecture overhaul and the intended module boundaries.
Each task should be recorded in the rolling context digest once completed.

## Goals

- Adopt a Session Actor core with worker-based pipelines.
- Introduce ports/adapters to isolate Tauri and platform dependencies.
- Preserve existing UI commands and event payloads.
- Keep the codebase readable, testable, and ready for future rewrite features.

## Task Breakdown

### Task 1: Module boundaries and scaffolding

- Add `domain`, `application`, `pipeline`, `ports`, and `adapters` modules.
- Move transcript logic to `domain`.
- Define application command and event types.
- Define ports for UI events and platform operations.

### Task 2: Session Actor core

- Replace the shared mutex session state with a single actor loop.
- Ensure the actor is the sole writer of session state.
- Preserve session state transitions and UI event semantics.

### Task 3: Dictation pipeline workers

- Move dictation pipeline logic into `pipeline`.
- Keep audio capture, sherpa streaming, and whisper workers as dedicated tasks.
- Route pipeline updates to the actor as events.

### Task 4: Adapters and wiring

- Implement Tauri event emission and window controls as adapters.
- Update `AppState` and command handlers to use the new session service.
- Keep the command surface and event payloads unchanged.

### Task 5: Documentation and context

- Update architecture decisions and repository layout specs.
- Update implementation notes and work plan guidance.
- Record each completed task in `docs/spec/context/`.
