# Spec Index

This folder contains architecture notes, implementation workflows, and context digests for AiR (a macOS 13+ “SuperWhisper-like” floating voice input assistant built with Rust + Tauri v2).

## Contents

Files use descriptive `snake_case` names. Read in this order if you are new:

- `docs/spec/architecture_decisions.md`: Tooling and architecture choices (including UI stack and constraints).
- `docs/spec/repository_layout.md`: Repository structure and module boundaries.
- `docs/spec/backend_ui_contract.md`: Backend events, frontend commands, payload schemas, and signatures.
- `docs/spec/setup_and_configuration.md`: Setup and configuration snippets (Tauri, permissions, UI tooling).
- `docs/spec/speech_to_text.md`: Speech-to-text integration notes (sherpa streaming + whisper second pass).
- `docs/spec/stt_twopass_sliding_window.md`: Working STT architecture spec (two-pass + sliding-window refinement).
- `docs/spec/stt_dictation_pipeline_spec.md`: Canonical STT pipeline spec v2 (sherpa streaming + whisper-window refinement + whisper second pass).
- `docs/spec/stt_dictation_pipeline_impl_notes.md`: Implementation notes for the canonical v2 dictation pipeline.
- `docs/spec/stt_dictation_pipeline_debugging.md`: Reproducible debugging workflow (trace + WAV capture).
- `docs/spec/stt_dictation_trace_regressions.md`: Trace-based regressions and fixes for dictation v2.
- `docs/spec/stt_comparison_harness.md`: STT A/B comparison harness usage (`stt-compare`).
- `docs/spec/roadmap.md`: Milestone-by-milestone plan and acceptance checks.
- `docs/spec/work_plan.md`: Contributor guide (where to start, where to change code).
- `docs/spec/architecture_upgrade_plan.md`: Task breakdown for the session actor + ports/adapters refactor.

## Scope

- Target: macOS 13+ only for the first implementation.
- Architecture: Must preserve platform boundaries for future Windows/Linux support (traits + adapters + feature flags).

## Conventions

- Backend emits events; frontend is a pure view/controller. The Rust backend is the single source of truth for the session state machine.
- Safety: No silent failures. Propagate or emit structured errors with actionable messages.
