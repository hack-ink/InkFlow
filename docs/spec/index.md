# Spec Index

Purpose: Provide the entry point for system specifications and contracts.

Audience: Engineers and LLMs reading the canonical system spec.

Scope: Architecture, backend/UI contracts, and speech-to-text behavior.

Documentation governance is defined in `docs/governance.md`.

## How to use this index

- Start with `docs/spec/architecture.md` for system boundaries and design decisions.
- Use `docs/spec/backend_ui_contract.md` for event and command contracts.
- Use `docs/spec/speech_to_text.md` for the STT overview and defaults.
- Use `docs/spec/stt_dictation_pipeline.md` for the canonical dictation pipeline details.

## Reading order

1. `docs/spec/architecture.md`
2. `docs/spec/backend_ui_contract.md`
3. `docs/spec/speech_to_text.md`
4. `docs/spec/stt_dictation_pipeline.md`

## Spec list

- `docs/spec/architecture.md`: Architecture decisions, repository layout, and system boundaries.
- `docs/spec/backend_ui_contract.md`: Backend events, frontend commands, payload schemas, and signatures.
- `docs/spec/speech_to_text.md`: Speech-to-text integration notes (sherpa streaming + Whisper second pass).
- `docs/spec/stt_dictation_pipeline.md`: Canonical dictation pipeline spec v2 (streaming + sliding-window + second pass).

Operational workflows and debugging runbooks live under `docs/guide/`.
