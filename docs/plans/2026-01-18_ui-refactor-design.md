# UI Refactor Design

Date: 2026-01-18.
Status: Approved requirements.

## Purpose
Define a structural refactor of the macOS SwiftUI UI to improve maintainability, reduce duplication, and improve performance while keeping the final visuals identical.

## Scope
- macOS SwiftUI only under `apps/macos/InkFlow/InkFlow`.
- Floating panel UI, settings UI, and shared visual components.
- Xcode project updates to reflect new physical paths.

## Goals
- Create a clear, domain-oriented file structure that is easy to navigate and extend.
- Reduce repeated layout and styling code through shared components and style files.
- Improve rendering performance without altering visual output.
- Keep UI tokens centralized in `UIConstants.swift`.

## Non-goals
- Visual redesign or new UI features.
- Backend or Rust changes.
- Changes to panel behavior, settings persistence, or app capabilities.

## Constraints
- Final visual output must be identical to the current UI.
- Behavior should remain unchanged unless a refactor requires a minimal, equivalent adjustment.
- No new public APIs unless strictly required by the refactor.

## Architecture and file organization
- Introduce a new top-level UI structure inside `apps/macos/InkFlow/InkFlow/UI`:
  - `UI/Panel` for the floating panel and its subviews.
  - `UI/Settings` for settings UI views and section components.
  - `UI/Shared` for reusable controls and view modifiers.
  - `UI/Styles` for domain-specific layout/style mappings.
- Move existing files to domain folders and keep entry points thin:
  - `ContentView.swift` -> `UI/Panel/PanelRootView.swift`.
  - `SettingsView.swift` -> `UI/Settings/SettingsRootView.swift`.
  - `ActivationOrbView.swift` -> `UI/Panel/Orb/ActivationOrbView.swift`.
  - `WaveformView.swift` -> `UI/Panel/Waveform/WaveformView.swift`.
  - `PanelHostViewController.swift` stays within `UI/Panel` as the AppKit bridge.
- Keep `UIConstants.swift` as the global tokens file; avoid adding new magic numbers outside layout enums.
- Add `UI/Styles/PanelStyles.swift` and `UI/Styles/SettingsStyles.swift` for local layout enums that map to global tokens.

## Components and duplication removal
- Panel UI decomposition:
  - `PanelHeaderView` (orb, transcript, expand button).
  - `PanelExpandedView` (mode tabs, module content, settings overlay).
  - `PanelModeTabsView` and `ModeTab` as explicit, reusable components.
  - `PanelSettingsSheetView` for the settings overlay container.
- Settings UI decomposition:
  - `SettingsSidebarView` and `SettingsSidebarButton`.
  - `SettingsSectionHeaderView`, `SettingsGroupView`, and per-section content views.
  - `SettingsShortcutRow` and `SettingsLevelMeterView` moved to `UI/Shared` where applicable.
- Shared elements:
  - Glass containers, icon buttons, selection rings, hover backgrounds, and common row layouts in `UI/Shared`.
  - Shared components should be small, pure, and style-driven to avoid duplication.

## Data flow and state
- Centralize appearance state near the root of the UI (panel and settings roots) and pass it down through environment or explicit parameters.
- Maintain existing state sources:
  - `PanelController` and `InkFlowViewModel` remain the panel sources of truth.
  - Settings state continues to use `@AppStorage` and section-specific view models where already in use.
- Preserve all existing bindings and behavior semantics when moving views.

## Performance pass
- Narrow animation scopes to the smallest possible subtrees to avoid broad recomputation.
- Reuse buffers in the orb renderer to reduce per-frame allocations without altering rendering order, colors, or alpha.
- Avoid repeated construction of gradients and layout calculations inside frequently updated views.
- Keep `TimelineView` cadence and particle math unchanged to preserve the animation look.

## Error handling
- Preserve existing error messaging for the panel and microphone test.
- Avoid silent failures; propagate errors to the same UI surfaces as before.
- Maintain no-`unwrap` policy in non-test code and avoid blocking in async paths.

## Visual verification checklist
- Panel collapsed and expanded states.
- Listening vs. idle orb appearance.
- Transcript placeholder vs. active text.
- Settings overlay appearance and dismissal behavior.
- Settings sections: Appearance, Microphone, Shortcuts.
- Hover and selection states in settings sidebar.

## Implementation phases
1. **Reorganization (C):** Create new UI folders, move files, and update `project.pbxproj`.
2. **Componentization (B):** Split large views into named subcomponents and shared views.
3. **Performance (A):** Localize animations, reuse buffers, and reduce allocations.

