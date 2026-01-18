# Floating Panel UI Notes

Purpose: Capture UI decisions and deferred enhancements for the macOS floating panel.

## Current behavior

- The panel is a Spotlight-style floating surface that can be repositioned by dragging empty space.
- The panel hides when it loses key focus and can be reopened from the menubar, dock, or shortcuts.
- The transcript is presented as a single line with truncation.
- The header includes the activation orb, transcript text, and an expand control.
- The expanded area hosts mode tabs and a settings overlay.
- A waveform module renders bar-style input levels while listening.
- The activation control uses a 3D particle letter morph that rotates continuously; transitions take half a rotation and finish exactly when the letter faces forward. It uses red (listening) and blue (idle) palettes.

## Implementation lessons

- Prefer system window dragging (`isMovableByWindowBackground`) over custom hit testing.
- Avoid overriding `NSHostingView` hit testing for drag behavior. It causes SwiftUI controls to stop receiving clicks.
- Keep the drag behavior in AppKit and keep SwiftUI views focused on interaction and visuals.

## UI constants and layout tokens

- Centralize layout values in `apps/macos/InkFlow/InkFlow/UIConstants.swift`.
- Prefer named layout enums in each view (for example: `HeaderLayout`, `SettingsLayout`) that map to shared tokens.
- Keep spacing, sizes, and animation durations readable and consistent. Avoid new magic numbers unless required.
- For the header, align the activation orb with the transcript text start using:
  - `leadingInset = rowSpacing + transcriptHorizontalPadding - panelPadding + orbOpticalInset`.
  - Adjust `orbOpticalInset` for optical balance if the particle glow feels left-heavy.
- In collapsed mode, center the header vertically by using `(collapsedHeight - headerHeight) / 2` for the header top inset.

## Activation orb layout

- The particle orb centers by its visible particle bounds, not by raw point averages. This keeps the visual center stable across letter morphs.
- Keep particle centering independent of header layout. The orb frame stays centered in the header, and particle centering happens inside the orb.

## Debug toggles

- Debug builds expose an Appearance > Debug setting for “Show orb frame.” This toggles the AppStorage key `debug.showOrbFrame`.

## Deferred enhancements

- Add transcript display modes: single-line (default), auto-grow to a capped number of lines, and fully expanded.
- Add a settings popover to select the display mode.
- Extend the waveform module with additional styles (for example: mirrored bars, dots, or a continuous line).
- Consider strengthening the particle orb effect with higher particle density, spark trails, or audio-reactive intensity.
