# Voice Dictation App Experience

Purpose: Capture product and UI requirements for the macOS dictation experience.

Scope: User-facing behavior, settings, and UI surfaces for dictation, rewriting, history,
and metrics. Detailed ASR pipeline mechanics live in `docs/spec/core/`.

## Product intent

- Provide a voice-first input experience that reduces dependence on keyboard input.
- Keep dictation lightweight and fast, with minimal interruption to the current task.

## Core behavior requirements

### Focus and target application

- The app must detect the frontmost target application when the dictation panel is invoked.
- Dictation should not steal focus from the target application unless explicitly requested.
- The final output should be delivered to the target application without disrupting the user's
  workflow.

### Rewrite pipeline

- ASR output must not be inserted directly into the target application.
- All ASR output is passed through an AI rewrite step before final delivery.
- Users can toggle whether rewriting happens automatically after ASR completion.

### Prompt configuration and merging

- Users can configure a per-application rewrite prompt.
- Users can configure global style prompts (tone, voice, or rewrite style).
- Per-application prompts and global style prompts must be merged for the rewrite request.
- The input UI must allow fast switching between style prompts with smooth visual feedback.

### Dictation method configuration

- The app must surface configuration for dictation method combinations defined in
  `docs/spec/core/stt_dictation_pipeline.md`.
- The UI should expose a clear selection for modes such as streaming-only, local two-pass,
  and local one-shot.

## UI surfaces and navigation

### Floating dictation panel

- The floating panel is the primary dictation surface and should remain compact and
  unobtrusive.
- The panel must expand vertically to accommodate longer transcript text, with smooth
  height animation and reflow.
- Newly recognized ASR segments should appear with subtle, smooth animations.
- A waveform or audio activity visualization should be present while listening. It should
  be elegant, readable, and not visually dominant.

### Settings and configuration

- Provide a dedicated surface for settings that does not overwhelm the dictation panel.
- Settings must include:
  - Per-application rewrite prompts.
  - Global style prompt presets.
  - Shortcut bindings (e.g., open panel, toggle dictation).
  - Dictation method configuration.

### History and metrics

- Provide a history view showing past dictation outputs with the ability to review
  the associated audio snippets.
- Provide a status/metrics view showing usage statistics such as:
  - Token consumption.
  - Total usage time.
  - Total ASR character/word count.

## Open questions

- Where should rewritten text be displayed before insertion: in the dictation panel, a
  secondary view, or a separate review area?
- Should history and metrics live in a unified dashboard or separate views?
