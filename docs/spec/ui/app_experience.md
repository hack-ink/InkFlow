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
- Rewriting occurs only after dictation is complete; real-time rewriting is not used.
- Automatic rewriting is controlled by a configuration toggle after ASR completion.
- Detailed automatic rewrite behavior is deferred and tracked as an issue.

### Prompt configuration and merging

- Users can configure a per-application rewrite prompt.
- Users can configure global style prompts (tone, voice, or rewrite style).
- Per-application prompts and global style prompts must be merged for the rewrite request.
- The input UI must allow fast switching between style prompts with smooth visual feedback.

### Dictation completion

- The app must support a hold-to-talk mode where releasing the key ends dictation.
- The app must support a press-to-toggle mode where the first press starts dictation and the
  next press ends dictation.

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

### History and metrics

- Provide a history view showing past dictation outputs with the ability to review
  the associated audio snippets.
- Provide a status/metrics view showing usage statistics such as:
  - Token consumption.
  - Total usage time.
  - Total ASR character/word count.
- History retention defaults to indefinite storage, pending a later review of privacy and
  cleanup controls.

## Open questions

- Where should rewritten text be displayed before insertion: in the dictation panel, a
  secondary view, or a separate review area?
- Should history and metrics live in a unified dashboard or separate views?

## Deferred items (track as issues)

- Define the rewrite presentation surface and interaction model.
- Defer the history experience implementation details.
- Finalize the automatic rewrite behavior for each dictation mode.
- Decide how to expose dictation method configuration (streaming-only, local two-pass, local
  one-shot) in settings.
- Revisit retention policy, privacy messaging, and cleanup controls.
