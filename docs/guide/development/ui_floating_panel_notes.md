# Floating Panel UI Notes

Purpose: Capture UI decisions and deferred enhancements for the macOS floating panel.

## Current behavior

- The transcript is presented as a single line with truncation.
- The panel uses player-style controls with play and stop icons, plus a secondary clear action.
- A waveform module renders bar-style input levels while listening.
- No settings panel is exposed yet.
- The activation control uses a 3D particle letter morph that rotates continuously; transitions take half a rotation and finish exactly when the letter faces forward. It uses red (listening) and blue (idle) palettes.

## Deferred enhancements

- Add transcript display modes: single-line (default), auto-grow to a capped number of lines, and fully expanded.
- Add a settings popover to select the display mode.
- Extend the waveform module with additional styles (for example: mirrored bars, dots, or a continuous line).
- Consider strengthening the particle orb effect with higher particle density, spark trails, or audio-reactive intensity.
