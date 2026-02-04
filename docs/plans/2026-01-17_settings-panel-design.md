# Settings Panel Design

Date: 2026-01-17.
Status: Approved requirements.

## Purpose
Define the settings entry points, navigation model, and section contents for InkFlow.

## Scope
- Menubar entry and settings window.
- Sections: Appearance, Microphone, Shortcuts.

## Non-goals
- Backend audio pipeline changes.
- Shortcut persistence or command bindings.
- Advanced audio processing options (noise suppression, auto gain, sample rate).

## Entry points
- The menubar status item menu contains only:
  - Settings... (shows shortcut Command-,).
  - Quit (shows shortcut Command-Q).
- Settings... and Command-, open a dedicated settings window.

## Navigation
- The settings window uses a sidebar with three items.
- Sidebar order: Appearance, Microphone, Shortcuts.
- The window remembers the last selected section.

## Appearance (functional)
- Theme: Light / Dark / System.
- Accent color: selectable accent color for UI highlights.
- Glass intensity: Subtle / Standard / Vivid.
- Window translucency: On / Off (disables glass surfaces when Off).

Behavior:
- Changes apply immediately to the app UI.
- Glass intensity maps to visible material strength; API mapping is implementation-defined.

## Microphone (test functional, otherwise placeholder)
- Input device: dropdown list of available input devices.
- Input level meter: read-only real-time meter.
- Test input: button triggers a short listening session to verify capture.

Behavior:
- Test input only verifies capture and updates the meter.
- No audio is recorded or played back.
- Input device selection does not change capture behavior in this version.
- No advanced processing controls are exposed.

## Shortcuts (interactive placeholder)
- Toggle dictation.
- Push-to-talk.
- Paste last transcript.
- Reset to defaults.

Behavior:
- Controls are interactive and respond to focus and input.
- No placeholder messaging or disabled states are shown.
- No persistence or functional side effects in this version.

## Liquid Glass usage
- Use glass surfaces in the title bar and sidebar to establish hierarchy.
- Keep content areas readable with adequate contrast.
- Do not stack excessive glass layers in the detail pane.
