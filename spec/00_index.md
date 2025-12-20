# Spec Index

This folder contains architecture notes, implementation workflows, and context digests for AiR (a macOS 13+ “SuperWhisper-like” floating voice input assistant built with Rust + Tauri v2).

If `spec/README.md` exists, it is a short pointer to this document.

Documentation format rules are defined in `spec/RULES.md`.

## Contents

Files are prefixed with numbers to keep a stable reading order.

Number ranges are used to group document types:

- `10`–`39`: Architecture and contracts.
- `40`–`69`: Implementation notes, workflows, and tooling.
- `70`–`79`: Roadmap and acceptance criteria.
- `80`–`89`: Contributor guidance and working notes.
- `90`–`99`: Archived planning documents.

- `spec/10_architecture_decisions.md`: Tooling and architecture choices (including UI stack and constraints).
- `spec/20_repository_layout.md`: Repository structure and module boundaries.
- `spec/30_backend_ui_contract.md`: Backend events, frontend commands, payload schemas, and signatures.
- `spec/40_setup_and_configuration.md`: Setup and configuration snippets (Tauri, permissions, UI tooling).
- `spec/50_speech_to_text.md`: Speech-to-text integration notes (sherpa streaming + whisper second pass).
- `spec/51_stt_twopass_sliding_window.md`: Working STT architecture spec (two-pass + sliding-window refinement).
- `spec/52_stt_dictation_pipeline_spec.md`: Canonical STT pipeline spec v2 (sherpa streaming + whisper-window refinement + whisper second pass).
- `spec/53_stt_dictation_pipeline_impl_notes.md`: Implementation notes for the canonical v2 dictation pipeline.
- `spec/54_stt_dictation_pipeline_debugging.md`: Reproducible debugging workflow (trace + WAV capture).
- `spec/55_stt_dictation_trace_regressions.md`: Trace-based regressions and fixes for dictation v2.
- `spec/56_stt_comparison_harness.md`: STT A/B comparison harness usage (`stt-compare`).
- `spec/70_roadmap.md`: Milestone-by-milestone plan and acceptance checks.
- `spec/80_work_plan.md`: Contributor guide (where to start, where to change code).
- `spec/81_architecture_upgrade_plan.md`: Task breakdown for the session actor + ports/adapters refactor.
- `spec/archive/file_by_file_implementation_checklist.md`: Archived file-by-file implementation checklist.
- Context digests use rolling 7-day windows starting from the first recorded entry.
- `spec/context/00_2025-12-15_to_2025-12-21_digest.md`: Consolidated context notes for fast retrieval after context limits.

## Scope

- Target: macOS 13+ only for the first implementation.
- Architecture: Must preserve platform boundaries for future Windows/Linux support (traits + adapters + feature flags).

## Conventions

- Backend emits events; frontend is a pure view/controller. The Rust backend is the single source of truth for the session state machine.
- Safety: No silent failures. Propagate or emit structured errors with actionable messages.
