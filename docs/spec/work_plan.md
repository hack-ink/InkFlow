# Work Plan (Contributor Guide)

This document is a high-level guide for implementing changes without needing to read a file-by-file checklist first.

## What to Read First

- Start with `docs/spec/index.md` for the full map of documents.
- Read `docs/spec/architecture_decisions.md` to understand constraints and non-goals.
- Read `docs/spec/backend_ui_contract.md` before changing events or commands.

## End-to-End Flow (Mental Model)

1. A global hotkey shows the overlay window.
2. Push-to-talk starts dictation and streams partial text updates to the UI.
3. Finalization produces a stable final transcript and emits `stt/final`.
4. Optional steps (rewrite and injection) consume that final transcript.

The backend is the single source of truth for the session state machine.

## Where to Look in Code

Backend entry points:

- Session orchestration: `src-tauri/src/application/session_actor.rs`.
- Session public facade: `src-tauri/src/session.rs`.
- Domain transcript logic: `src-tauri/src/domain/transcript.rs`.
- Event and payload definitions: `src-tauri/src/events.rs`.
- UI commands: `src-tauri/src/commands.rs`.

Speech-to-text:

- Pipeline orchestration: `src-tauri/src/pipeline/dictation.rs`.
- STT integration and config resolution: `src-tauri/src/stt.rs`.
- Whisper integration: `src-tauri/src/stt/whisper.rs`.
- Microphone capture: `src-tauri/src/audio/mic_stream.rs`.

Frontend entry point:

- Overlay UI and event wiring: `ui/src/App.tsx`.

## How to Add or Update Spec and Context

- Update an existing spec document if the change modifies architecture, contracts, configuration, or workflows.
- Add a new context entry by appending a short summary to the correct rolling 7-day digest under `docs/spec/context/`.
- Follow the formatting rules in `docs/spec/RULES.md`.

## Common Change Types

- Architecture and boundaries: update `docs/spec/architecture_decisions.md` and `docs/spec/repository_layout.md`.
- Events and commands: update `docs/spec/backend_ui_contract.md`.
- Setup, permissions, and configuration: update `docs/spec/setup_and_configuration.md`.
- Speech-to-text behavior: update `docs/spec/speech_to_text.md` and, when relevant, `docs/spec/stt_dictation_pipeline_spec.md`.
- STT measurement and tuning: update `docs/spec/stt_comparison_harness.md`.

## Deep Dive Reference

If you need a detailed, file-by-file planning document, see `docs/spec/archive/file_by_file_implementation_checklist.md`.
