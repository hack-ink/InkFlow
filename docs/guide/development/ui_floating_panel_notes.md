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

## Deferred enhancements

- Add transcript display modes: single-line (default), auto-grow to a capped number of lines, and fully expanded.
- Add a settings popover to select the display mode.
- Extend the waveform module with additional styles (for example: mirrored bars, dots, or a continuous line).
- Consider strengthening the particle orb effect with higher particle density, spark trails, or audio-reactive intensity.
