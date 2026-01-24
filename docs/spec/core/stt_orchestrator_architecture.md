# STT Orchestrator Architecture (Target)

Purpose: Define the target STT backend architecture that prioritizes extensibility, high-quality
transcripts, and smooth UI updates while keeping responsibilities clear and LLM-friendly.

Audience: Engineers and LLMs working on the speech pipeline, as well as UI engineers consuming
transcript updates.

Scope: Core abstractions, data flow, and UI contract expectations. Implementation details remain
in `docs/spec/core/stt_dictation_pipeline.md`.

## Goals

- Make speech pipelines composable without rewriting UI logic.
- Separate scheduling decisions from transcript semantics.
- Keep a single source of truth for displayed text.
- Support future local and API decoders without changing the UI contract.

## Core abstractions

### Orchestrator

The Orchestrator owns the end-to-end speech pipeline. It receives audio frames, delegates work to
Decoders via the Scheduler, and emits `RenderUpdate` events that the UI renders directly.

### Decoder

A Decoder is any speech engine or mode that can produce hypotheses. Each Decoder emits a uniform
`Hypothesis` output, regardless of whether it is streaming, windowed, second-pass, batch, or API
backed. Decoders do not read UI state and do not talk to each other.

### Scheduler

The Scheduler is a policy engine. It decides when to run each Decoder based on priority, budgets,
backpressure, and latency targets. It does not interpret transcript semantics.

### Fusion Engine

The Fusion Engine is deterministic and owns transcript semantics. It consumes hypotheses and
produces a single `RenderState` for the UI. It is responsible for de-duplication, stability, and
replacement of provisional text by higher-quality results.

## Data flow

1. Audio frames enter the Orchestrator.
2. The Scheduler dispatches work to active Decoders.
3. Decoders emit `Hypothesis` events.
4. The Fusion Engine merges hypotheses into a `RenderState`.
5. The Orchestrator emits `RenderUpdate` events to the UI.

The UI renders `RenderUpdate` and does not interpret raw decoder outputs.

## Canonical data types

### Hypothesis

- `source`: Origin of the hypothesis (stream, window, second-pass, batch, API).
- `segments`: List of `{ t0_ms, t1_ms, text, confidence, lang }`.
- `window`: Optional metadata for windowed decoders (end sample, generation, job id).
- `quality`: Normalized score for fusion ordering.
- `flags`: `{ is_partial, is_final, is_replacement }`.

### RenderState

- `segments`: Committed segments with provenance and timestamps.
- `live_tail`: Current in-progress tail text.
- `display_text`: Full user-facing string composed from segments plus tail.

### RenderUpdate

- `display_text`: Single string used by the UI.
- Optional `segments_delta`: Replace or insert committed segments for history views.
- RenderUpdate corresponds to the `render_update` (`live_render`) event in
  `docs/spec/ui/backend_ui_contract.md`.

## Stability and replacement rules (high-level)

- Window hypotheses may update only the live tail.
- Second-pass or batch hypotheses may replace committed segments deterministically.
- If no higher-quality result arrives, the Fusion Engine holds the last stable output instead of
  falling back to lower-quality text.
- Overlap resolution must be time-based to avoid CJK tokenization issues.

## Relationship to current implementation

The current pipeline (local stream + local window + local second pass) maps directly to this
architecture. The Orchestrator is the owner, and each decoding mode becomes a Decoder with a
uniform hypothesis output. The Scheduler and Fusion Engine define policy and transcript logic
without UI dependencies.
