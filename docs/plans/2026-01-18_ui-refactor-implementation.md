# UI Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refactor the macOS SwiftUI UI into a clear domain structure, reduce duplication, and improve performance while preserving identical visuals.

**Architecture:** Move UI files into `UI/Panel`, `UI/Settings`, `UI/Shared`, and `UI/Styles`. Keep root views thin and pass appearance state down to subviews. Performance work focuses on narrowing update scopes and reusing buffers in the orb renderer without changing rendering output.

**Tech Stack:** SwiftUI, AppKit, Combine.

---

## Notes
- Follow @swiftui-view-refactor ordering rules for all SwiftUI view files.
- Apply @swiftui-performance-audit guidance to reduce update churn.
- Per user request, do not create commits. Document any logical commit points for future reference, but skip the actual commit steps.
- The plan assumes work happens on `main` (no worktree), which conflicts with the writing-plans guideline. Obey the user request.

---

### Task 1: Create UI folder structure and move existing files

**Files:**
- Create: `apps/macos/InkFlow/InkFlow/UI/Panel`
- Create: `apps/macos/InkFlow/InkFlow/UI/Panel/Orb`
- Create: `apps/macos/InkFlow/InkFlow/UI/Panel/Waveform`
- Create: `apps/macos/InkFlow/InkFlow/UI/Settings`
- Create: `apps/macos/InkFlow/InkFlow/UI/Shared`
- Create: `apps/macos/InkFlow/InkFlow/UI/Styles`
- Move: `apps/macos/InkFlow/InkFlow/ContentView.swift` -> `apps/macos/InkFlow/InkFlow/UI/Panel/PanelRootView.swift`
- Move: `apps/macos/InkFlow/InkFlow/SettingsView.swift` -> `apps/macos/InkFlow/InkFlow/UI/Settings/SettingsRootView.swift`
- Move: `apps/macos/InkFlow/InkFlow/ActivationOrbView.swift` -> `apps/macos/InkFlow/InkFlow/UI/Panel/Orb/ActivationOrbView.swift`
- Move: `apps/macos/InkFlow/InkFlow/WaveformView.swift` -> `apps/macos/InkFlow/InkFlow/UI/Panel/Waveform/WaveformView.swift`
- Move: `apps/macos/InkFlow/InkFlow/PanelHostViewController.swift` -> `apps/macos/InkFlow/InkFlow/UI/Panel/PanelHostViewController.swift`
- Modify: `apps/macos/InkFlow/InkFlow.xcodeproj/project.pbxproj`

**Step 1: Create folders**

Run:
```bash
mkdir -p apps/macos/InkFlow/InkFlow/UI/Panel/Orb
mkdir -p apps/macos/InkFlow/InkFlow/UI/Panel/Waveform
mkdir -p apps/macos/InkFlow/InkFlow/UI/Settings
mkdir -p apps/macos/InkFlow/InkFlow/UI/Shared
mkdir -p apps/macos/InkFlow/InkFlow/UI/Styles
```
Expected: New directories exist under `apps/macos/InkFlow/InkFlow/UI`.

**Step 2: Move files into new structure**

Run:
```bash
mv apps/macos/InkFlow/InkFlow/ContentView.swift apps/macos/InkFlow/InkFlow/UI/Panel/PanelRootView.swift
mv apps/macos/InkFlow/InkFlow/SettingsView.swift apps/macos/InkFlow/InkFlow/UI/Settings/SettingsRootView.swift
mv apps/macos/InkFlow/InkFlow/ActivationOrbView.swift apps/macos/InkFlow/InkFlow/UI/Panel/Orb/ActivationOrbView.swift
mv apps/macos/InkFlow/InkFlow/WaveformView.swift apps/macos/InkFlow/InkFlow/UI/Panel/Waveform/WaveformView.swift
mv apps/macos/InkFlow/InkFlow/PanelHostViewController.swift apps/macos/InkFlow/InkFlow/UI/Panel/PanelHostViewController.swift
```
Expected: Files are present at their new paths.

**Step 3: Update Xcode project file paths**

Modify `apps/macos/InkFlow/InkFlow.xcodeproj/project.pbxproj` so file references and build phase entries point to the new paths. Ensure new group structure reflects the `UI` folders.

**Step 4: Update Swift references for renamed files**

Search for file references and ensure any type names (if renamed in later tasks) are updated. At this step, keep types the same and only adjust filenames as needed.

**Step 5: Skip commit**

Per user request, do not commit.

---

### Task 2: Introduce panel root and split panel subviews

**Files:**
- Create: `apps/macos/InkFlow/InkFlow/UI/Panel/PanelBackgroundView.swift`
- Create: `apps/macos/InkFlow/InkFlow/UI/Panel/PanelHeaderView.swift`
- Create: `apps/macos/InkFlow/InkFlow/UI/Panel/PanelExpandedView.swift`
- Create: `apps/macos/InkFlow/InkFlow/UI/Panel/PanelModeTabsView.swift`
- Create: `apps/macos/InkFlow/InkFlow/UI/Panel/PanelSettingsSheetView.swift`
- Modify: `apps/macos/InkFlow/InkFlow/UI/Panel/PanelRootView.swift`
- Modify: `apps/macos/InkFlow/InkFlow/UI/Panel/PanelHostViewController.swift`
- Create: `apps/macos/InkFlow/InkFlow/UI/Styles/PanelStyles.swift`

**Step 1: Add panel layout styles**

Create `apps/macos/InkFlow/InkFlow/UI/Styles/PanelStyles.swift`:
```swift
import SwiftUI

enum PanelHeaderLayout {
    static let rowSpacing: CGFloat = 9
    static let rowHeight: CGFloat = UIPanelLayout.headerHeight
    static let leadingSpacing: CGFloat = UISpacing.medium
    static let transcriptVerticalPadding: CGFloat = UISpacing.xxSmall
    static let transcriptHorizontalPadding: CGFloat = UISpacing.xSmall
    static let transcriptHeight: CGFloat = UISize.transcriptHeight
    static let expandButtonSize: CGFloat = UISize.iconSmall
    static let waveformHeight: CGFloat = UISize.waveformHeight
    static let waveformActiveOpacity: Double = 0.12
    static let waveformInactiveOpacity: Double = 0.05
    static let waveformBlurRadius: CGFloat = 0.4
    static let waveformMaskOpacity: Double = 0.9
    static let orbOpticalInset: CGFloat = UISpacing.xxSmall
    static let leadingInset: CGFloat = max(
        0,
        rowSpacing + transcriptHorizontalPadding - UIPanelLayout.padding + orbOpticalInset
    )
}

enum PanelExpandedLayout {
    static let stackSpacing: CGFloat = UISpacing.medium
    static let modeBarSpacing: CGFloat = UISpacing.xLarge
    static let settingsButtonSize: CGFloat = UISize.iconMedium
    static let settingsButtonTopPadding: CGFloat = UISpacing.xxSmall
    static let dividerOpacity: Double = 0.35
}

enum PanelSettingsSheetLayout {
    static let width: CGFloat = 520
    static let height: CGFloat = 280
    static let padding: CGFloat = UISpacing.xLarge
    static let stackSpacing: CGFloat = UISpacing.large
    static let translucentOpacity: Double = 0.95
}

enum PanelModeTabLayout {
    static let spacing: CGFloat = UISpacing.xSmall
}
```

**Step 2: Create `PanelBackgroundView`**

Create `apps/macos/InkFlow/InkFlow/UI/Panel/PanelBackgroundView.swift`:
```swift
import SwiftUI

struct PanelBackgroundView: View {
    @ObservedObject var panelController: PanelController
    let appearance: AppearanceStyle

    var body: some View {
        GeometryReader { _ in
            let collapsedRadius = max(0, panelController.collapsedPanelHeight / 2)
            let cornerRadius = panelController.isExpanded ? UIPanelLayout.expandedCornerRadius : collapsedRadius
            let shape = RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
            Group {
                if #available(macOS 26.0, *), appearance.isTranslucent {
                    GlassEffectContainer(spacing: UISpacing.medium) {
                        Color.clear
                            .frame(maxWidth: .infinity, maxHeight: .infinity)
                            .glassEffect(.regular.tint(appearance.surfaceTint), in: .rect(cornerRadius: cornerRadius))
                    }
                } else if appearance.isTranslucent {
                    shape.fill(.ultraThinMaterial)
                } else {
                    shape.fill(Color(nsColor: .windowBackgroundColor))
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .allowsHitTesting(false)
            .animation(.easeInOut(duration: UIDuration.panelExpand), value: panelController.isExpanded)
        }
    }
}
```

**Step 3: Create `PanelHeaderView`**

Create `apps/macos/InkFlow/InkFlow/UI/Panel/PanelHeaderView.swift`:
```swift
import SwiftUI

struct PanelHeaderView: View {
    @ObservedObject var model: InkFlowViewModel
    @ObservedObject var panelController: PanelController
    let appearance: AppearanceStyle

    var body: some View {
        headerRow
            .preferredColorScheme(appearance.preferredColorScheme)
            .tint(appearance.accentColor)
            .onExitCommand { panelController.handleExitCommand() }
    }

    private var headerRow: some View {
        HStack(alignment: .center, spacing: PanelHeaderLayout.rowSpacing) {
            leadingBlock
            transcriptStrip
            expandButton
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .frame(height: PanelHeaderLayout.rowHeight, alignment: .center)
        .textSelection(.disabled)
    }

    private var leadingBlock: some View {
        HStack(spacing: PanelHeaderLayout.leadingSpacing) {
            statusGlyph
            if let error = model.errorMessage {
                Text(error)
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(UIColors.errorText)
            }
        }
        .padding(.leading, PanelHeaderLayout.leadingInset)
    }

    private var statusGlyph: some View {
        Button(action: toggleListening) {
            ActivationOrbView(isActive: model.isListening)
        }
        .buttonStyle(.plain)
        .focusable(false)
    }

    private var transcriptStrip: some View {
        ZStack(alignment: .leading) {
            waveformBackdrop
            Text(model.transcript.isEmpty ? "Speak to dictate." : model.transcript)
                .font(.system(size: 16, weight: .medium))
                .foregroundStyle(.primary)
                .lineLimit(1)
                .truncationMode(.tail)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.vertical, PanelHeaderLayout.transcriptVerticalPadding)
        .padding(.horizontal, PanelHeaderLayout.transcriptHorizontalPadding)
        .frame(height: PanelHeaderLayout.transcriptHeight)
        .textSelection(.disabled)
        .allowsHitTesting(false)
    }

    private var expandButton: some View {
        Button(action: panelController.toggleExpanded) {
            Image(systemName: panelController.isExpanded ? "chevron.up" : "chevron.down")
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(.secondary)
                .frame(width: PanelHeaderLayout.expandButtonSize, height: PanelHeaderLayout.expandButtonSize)
        }
        .buttonStyle(.plain)
        .focusable(false)
        .animation(.easeInOut(duration: UIDuration.standard), value: panelController.isExpanded)
        .accessibilityLabel(panelController.isExpanded ? "Collapse panel" : "Expand panel")
    }

    private var waveformBackdrop: some View {
        WaveformView(levels: model.waveformLevels, isActive: model.isListening)
            .opacity(model.isListening ? PanelHeaderLayout.waveformActiveOpacity : PanelHeaderLayout.waveformInactiveOpacity)
            .blur(radius: PanelHeaderLayout.waveformBlurRadius)
            .frame(height: PanelHeaderLayout.waveformHeight)
            .mask(waveformFadeMask)
    }

    private var waveformFadeMask: some View {
        LinearGradient(
            colors: [
                .clear,
                .white.opacity(PanelHeaderLayout.waveformMaskOpacity),
                .white.opacity(PanelHeaderLayout.waveformMaskOpacity),
                .clear
            ],
            startPoint: .leading,
            endPoint: .trailing
        )
    }

    private func toggleListening() {
        if model.isListening {
            model.stop()
        } else {
            model.start()
        }
    }
}
```

**Step 4: Create `PanelModeTabsView` and `PanelExpandedView`**

Create `apps/macos/InkFlow/InkFlow/UI/Panel/PanelModeTabsView.swift`:
```swift
import SwiftUI

enum PanelMode: String, CaseIterable, Identifiable {
    case history
    case clips
    case notes

    var id: String { rawValue }

    var title: String {
        switch self {
        case .history:
            return "History"
        case .clips:
            return "Clips"
        case .notes:
            return "Notes"
        }
    }
}

struct PanelModeTabsView: View {
    let selectedMode: PanelMode
    let onSelect: (PanelMode) -> Void

    var body: some View {
        HStack(spacing: PanelExpandedLayout.modeBarSpacing) {
            ForEach(PanelMode.allCases) { mode in
                PanelModeTab(title: mode.title, isSelected: selectedMode == mode) {
                    withAnimation(.easeInOut(duration: UIDuration.selectionChange)) {
                        onSelect(mode)
                    }
                }
            }
            Spacer(minLength: 0)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

private struct PanelModeTab: View {
    let title: String
    let isSelected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            VStack(spacing: PanelModeTabLayout.spacing) {
                Text(title)
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(isSelected ? Color.primary : Color.secondary)
                    .lineLimit(1)
                    .truncationMode(.tail)
                Rectangle()
                    .frame(height: UISize.modeUnderlineHeight)
                    .foregroundStyle(isSelected ? UIColors.modeTabIndicatorSelected : UIColors.modeTabIndicatorUnselected)
            }
        }
        .buttonStyle(.plain)
        .animation(.easeInOut(duration: UIDuration.selectionChange), value: isSelected)
    }
}
```

Create `apps/macos/InkFlow/InkFlow/UI/Panel/PanelExpandedView.swift`:
```swift
import SwiftUI

struct PanelExpandedView: View {
    @ObservedObject var panelController: PanelController
    let appearance: AppearanceStyle
    @State private var selectedMode: PanelMode = .history

    var body: some View {
        ZStack(alignment: .topTrailing) {
            VStack(alignment: .leading, spacing: PanelExpandedLayout.stackSpacing) {
                PanelModeTabsView(selectedMode: selectedMode) { selectedMode = $0 }
                Divider().opacity(PanelExpandedLayout.dividerOpacity)
                moduleContent
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)

            settingsAccessory

            if panelController.isSettingsPresented {
                PanelSettingsSheetView(panelController: panelController, appearance: appearance)
                    .transition(.opacity.combined(with: .scale(scale: 0.98)))
            }
        }
        .preferredColorScheme(appearance.preferredColorScheme)
        .tint(appearance.accentColor)
        .onExitCommand { panelController.handleExitCommand() }
        .animation(.easeInOut(duration: UIDuration.panelSettingsToggle), value: panelController.isSettingsPresented)
    }

    private var settingsAccessory: some View {
        Button(action: panelController.toggleSettings) {
            Image(systemName: "slider.horizontal.3")
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(panelController.isSettingsPresented ? Color.primary : Color.secondary)
                .frame(width: PanelExpandedLayout.settingsButtonSize, height: PanelExpandedLayout.settingsButtonSize)
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Settings")
        .padding(.top, PanelExpandedLayout.settingsButtonTopPadding)
    }

    private var moduleContent: some View {
        Color.clear
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}
```

**Step 5: Create `PanelSettingsSheetView`**

Create `apps/macos/InkFlow/InkFlow/UI/Panel/PanelSettingsSheetView.swift`:
```swift
import SwiftUI

struct PanelSettingsSheetView: View {
    @ObservedObject var panelController: PanelController
    let appearance: AppearanceStyle

    var body: some View {
        ZStack {
            UIColors.overlayScrim
                .contentShape(Rectangle())
                .onTapGesture { panelController.closeSettings() }
            settingsSheet
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var settingsSheet: some View {
        VStack(spacing: PanelSettingsSheetLayout.stackSpacing) {
            HStack {
                Text("Settings")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(.primary)
                Spacer()
                Button(action: panelController.closeSettings) {
                    Image(systemName: "xmark")
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(.secondary)
                        .frame(width: UISize.iconSmall, height: UISize.iconSmall)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Close settings")
            }

            SettingsRootView()
        }
        .padding(PanelSettingsSheetLayout.padding)
        .frame(width: PanelSettingsSheetLayout.width, height: PanelSettingsSheetLayout.height, alignment: .topLeading)
        .background(settingsSheetBackground)
        .clipShape(RoundedRectangle(cornerRadius: UICornerRadius.medium, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: UICornerRadius.medium, style: .continuous)
                .strokeBorder(UIColors.settingsBorder)
        )
        .shadow(
            color: UIColors.settingsShadow,
            radius: UIShadow.settingsSheetRadius,
            x: UIShadow.settingsSheetX,
            y: UIShadow.settingsSheetY
        )
    }

    private var settingsSheetBackground: some View {
        Color(nsColor: .windowBackgroundColor).opacity(
            appearance.isTranslucent ? PanelSettingsSheetLayout.translucentOpacity : 1.0
        )
    }
}
```

**Step 6: Update `PanelRootView` and host controller**

Modify `apps/macos/InkFlow/InkFlow/UI/Panel/PanelRootView.swift` to only compose the panel views:
```swift
import SwiftUI

struct PanelRootView: View {
    @ObservedObject var model: InkFlowViewModel
    @ObservedObject var panelController: PanelController
    @AppStorage("appearance.theme") private var themeRaw = ThemePreference.system.rawValue
    @AppStorage("appearance.accent") private var accentRaw = AccentOption.sky.rawValue
    @AppStorage("appearance.glassIntensity") private var glassIntensityRaw = GlassIntensity.standard.rawValue
    @AppStorage("appearance.windowTranslucency") private var isWindowTranslucent = true

    var body: some View {
        let appearance = AppearanceStyle(
            theme: AppearanceStyle.theme(from: themeRaw),
            accent: AppearanceStyle.accent(from: accentRaw),
            glassIntensity: AppearanceStyle.glassIntensity(from: glassIntensityRaw),
            isTranslucent: isWindowTranslucent
        )
        ZStack {
            PanelBackgroundView(panelController: panelController, appearance: appearance)
            VStack(spacing: UIPanelLayout.headerSpacing) {
                PanelHeaderView(model: model, panelController: panelController, appearance: appearance)
                PanelExpandedView(panelController: panelController, appearance: appearance)
            }
            .padding(UIPanelLayout.padding)
        }
    }
}
```

Update `apps/macos/InkFlow/InkFlow/UI/Panel/PanelHostViewController.swift` to use `PanelRootView` for previews if needed, but keep the existing host setup. Ensure it continues to host `PanelBackgroundView`, `PanelHeaderView`, and `PanelExpandedView` with the new `appearance` parameter. Use an `AppearanceStyle` computed once when creating root views inside the host.

**Step 7: Skip commit**

Per user request, do not commit.

---

### Task 3: Split settings UI into sections and shared components

**Files:**
- Create: `apps/macos/InkFlow/InkFlow/UI/Settings/SettingsSidebarView.swift`
- Create: `apps/macos/InkFlow/InkFlow/UI/Settings/SettingsAppearanceSectionView.swift`
- Create: `apps/macos/InkFlow/InkFlow/UI/Settings/SettingsMicrophoneSectionView.swift`
- Create: `apps/macos/InkFlow/InkFlow/UI/Settings/SettingsShortcutsSectionView.swift`
- Create: `apps/macos/InkFlow/InkFlow/UI/Settings/SettingsSectionHeaderView.swift`
- Modify: `apps/macos/InkFlow/InkFlow/UI/Settings/SettingsRootView.swift`
- Create: `apps/macos/InkFlow/InkFlow/UI/Styles/SettingsStyles.swift`
- Create: `apps/macos/InkFlow/InkFlow/UI/Shared/SettingsGroupView.swift`
- Create: `apps/macos/InkFlow/InkFlow/UI/Shared/SettingsShortcutRow.swift`
- Create: `apps/macos/InkFlow/InkFlow/UI/Shared/SettingsLevelMeterView.swift`

**Step 1: Add settings layout styles**

Create `apps/macos/InkFlow/InkFlow/UI/Styles/SettingsStyles.swift`:
```swift
import SwiftUI

enum SettingsLayout {
    static let rootSpacing: CGFloat = UISpacing.xLarge
    static let sidebarSpacing: CGFloat = UISpacing.small
    static let sidebarWidth: CGFloat = 150
    static let sidebarItemVerticalPadding: CGFloat = UISpacing.small
    static let sidebarItemHorizontalPadding: CGFloat = UISpacing.medium
    static let sectionSpacing: CGFloat = UISpacing.xLarge
    static let groupSpacing: CGFloat = 10
    static let inlineSpacing: CGFloat = UISpacing.small
    static let scrollVerticalPadding: CGFloat = UISpacing.medium
    static let accentGridItemSize: CGFloat = 26
    static let accentGridSpacing: CGFloat = 10
    static let accentGridColumns: Int = 6
    static let shortcutLabelWidth: CGFloat = 170
    static let shortcutFieldVerticalPadding: CGFloat = 5
    static let shortcutFieldHorizontalPadding: CGFloat = UISpacing.medium
    static let selectionRingLineWidth: CGFloat = 2
}
```

**Step 2: Extract shared settings components**

Create `apps/macos/InkFlow/InkFlow/UI/Shared/SettingsGroupView.swift`:
```swift
import SwiftUI

struct SettingsGroupView<Content: View>: View {
    let title: String
    let content: Content

    init(title: String, @ViewBuilder content: () -> Content) {
        self.title = title
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: SettingsLayout.groupSpacing) {
            Text(title)
                .font(.caption.weight(.semibold))
                .foregroundStyle(.secondary)
            content
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
```

Create `apps/macos/InkFlow/InkFlow/UI/Shared/SettingsShortcutRow.swift`:
```swift
import SwiftUI

struct SettingsShortcutRow: View {
    let title: String
    @Binding var value: String

    var body: some View {
        HStack {
            Text(title)
                .frame(width: SettingsLayout.shortcutLabelWidth, alignment: .leading)
            TextField("", text: $value)
                .textFieldStyle(.plain)
                .font(.system(size: 13, weight: .medium, design: .monospaced))
                .padding(.vertical, SettingsLayout.shortcutFieldVerticalPadding)
                .padding(.horizontal, SettingsLayout.shortcutFieldHorizontalPadding)
                .background(
                    RoundedRectangle(cornerRadius: UICornerRadius.small, style: .continuous)
                        .fill(UIColors.shortcutFieldBackground)
                )
        }
    }
}
```

Create `apps/macos/InkFlow/InkFlow/UI/Shared/SettingsLevelMeterView.swift`:
```swift
import SwiftUI

struct SettingsLevelMeterView: View {
    let level: CGFloat
    let isActive: Bool

    var body: some View {
        GeometryReader { proxy in
            let width = proxy.size.width
            let height = proxy.size.height
            let filled = max(min(level, 1), 0) * width

            ZStack(alignment: .leading) {
                RoundedRectangle(cornerRadius: height / 2, style: .continuous)
                    .fill(UIColors.levelMeterTrack)
                RoundedRectangle(cornerRadius: height / 2, style: .continuous)
                    .fill(isActive ? UIColors.levelMeterActiveFill : UIColors.levelMeterInactiveFill)
                    .frame(width: filled)
            }
        }
        .frame(height: UISize.levelMeterHeight)
        .animation(.easeOut(duration: UIDuration.meterLevel), value: level)
        .animation(.easeInOut(duration: UIDuration.standard), value: isActive)
    }
}
```

**Step 3: Create settings sidebar**

Create `apps/macos/InkFlow/InkFlow/UI/Settings/SettingsSidebarView.swift`:
```swift
import SwiftUI

enum SettingsSection: String, CaseIterable, Identifiable {
    case appearance
    case microphone
    case shortcuts

    var id: String { rawValue }

    var title: String {
        switch self {
        case .appearance:
            return "Appearance"
        case .microphone:
            return "Microphone"
        case .shortcuts:
            return "Shortcuts"
        }
    }
}

struct SettingsSidebarView: View {
    let selectedSection: SettingsSection
    let onSelect: (SettingsSection) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: SettingsLayout.sidebarSpacing) {
            ForEach(SettingsSection.allCases) { section in
                SettingsSidebarButton(title: section.title, isSelected: selectedSection == section) {
                    withAnimation(.easeInOut(duration: UIDuration.selectionChange)) {
                        onSelect(section)
                    }
                }
            }
        }
        .frame(width: SettingsLayout.sidebarWidth, alignment: .leading)
    }
}

private struct SettingsSidebarButton: View {
    let title: String
    let isSelected: Bool
    let action: () -> Void
    @State private var isHovered = false

    var body: some View {
        Button(action: action) {
            Text(title)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(isSelected ? Color.primary : Color.secondary)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.vertical, SettingsLayout.sidebarItemVerticalPadding)
                .padding(.horizontal, SettingsLayout.sidebarItemHorizontalPadding)
                .background(background)
                .clipShape(RoundedRectangle(cornerRadius: UICornerRadius.small, style: .continuous))
        }
        .buttonStyle(.plain)
        .onHover { hovering in
            withAnimation(.easeInOut(duration: UIDuration.hoverFade)) {
                isHovered = hovering
            }
        }
    }

    @ViewBuilder
    private var background: some View {
        if isSelected {
            RoundedRectangle(cornerRadius: UICornerRadius.small, style: .continuous)
                .fill(UIColors.sidebarSelectedBackground)
        } else if isHovered {
            RoundedRectangle(cornerRadius: UICornerRadius.small, style: .continuous)
                .fill(UIColors.sidebarHoverBackground)
        } else {
            Color.clear
        }
    }
}
```

**Step 4: Create settings section views**

Create `apps/macos/InkFlow/InkFlow/UI/Settings/SettingsAppearanceSectionView.swift`:
```swift
import SwiftUI

struct SettingsAppearanceSectionView: View {
    @AppStorage("appearance.theme") private var themeRaw = ThemePreference.system.rawValue
    @AppStorage("appearance.accent") private var accentRaw = AccentOption.sky.rawValue
    @AppStorage("appearance.glassIntensity") private var glassIntensityRaw = GlassIntensity.standard.rawValue
    @AppStorage("appearance.windowTranslucency") private var isWindowTranslucent = true
#if DEBUG
    @AppStorage("debug.showOrbFrame") private var showOrbFrame = false
#endif

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: SettingsLayout.sectionSpacing) {
                SettingsGroupView(title: "Theme") {
                    Picker("Theme", selection: themeBinding) {
                        ForEach(ThemePreference.allCases) { option in
                            Text(option.title).tag(option)
                        }
                    }
                    .pickerStyle(.segmented)
                }
                Divider()
                SettingsGroupView(title: "Accent Color") {
                    LazyVGrid(
                        columns: Array(
                            repeating: GridItem(.fixed(SettingsLayout.accentGridItemSize), spacing: SettingsLayout.accentGridSpacing),
                            count: SettingsLayout.accentGridColumns
                        ),
                        spacing: SettingsLayout.accentGridSpacing
                    ) {
                        ForEach(AccentOption.allCases) { option in
                            Button {
                                withAnimation(.easeInOut(duration: UIDuration.selectionChange)) {
                                    accentRaw = option.rawValue
                                }
                            } label: {
                                Circle()
                                    .fill(option.color)
                                    .frame(width: UISize.accentSwatch, height: UISize.accentSwatch)
                                    .overlay(selectionRing(for: option))
                            }
                            .buttonStyle(.plain)
                            .accessibilityLabel(option.title)
                        }
                    }
                }
                Divider()
                SettingsGroupView(title: "Window and Glass") {
                    VStack(alignment: .leading, spacing: SettingsLayout.groupSpacing) {
                        Picker("Glass intensity", selection: glassIntensityBinding) {
                            ForEach(GlassIntensity.allCases) { option in
                                Text(option.title).tag(option)
                            }
                        }
                        .pickerStyle(.segmented)

                        Toggle("Window translucency", isOn: $isWindowTranslucent)
                    }
                }
#if DEBUG
                Divider()
                SettingsGroupView(title: "Debug") {
                    Toggle("Show orb frame", isOn: $showOrbFrame)
                }
#endif
            }
            .padding(.vertical, SettingsLayout.scrollVerticalPadding)
        }
    }

    private var themeBinding: Binding<ThemePreference> {
        Binding(
            get: { AppearanceStyle.theme(from: themeRaw) },
            set: { themeRaw = $0.rawValue }
        )
    }

    private var glassIntensityBinding: Binding<GlassIntensity> {
        Binding(
            get: { AppearanceStyle.glassIntensity(from: glassIntensityRaw) },
            set: { glassIntensityRaw = $0.rawValue }
        )
    }

    @ViewBuilder
    private func selectionRing(for option: AccentOption) -> some View {
        if AppearanceStyle.accent(from: accentRaw) == option {
            Circle()
                .strokeBorder(UIColors.selectionRing, lineWidth: SettingsLayout.selectionRingLineWidth)
        }
    }
}
```

Create `apps/macos/InkFlow/InkFlow/UI/Settings/SettingsMicrophoneSectionView.swift`:
```swift
import AVFoundation
import SwiftUI

struct SettingsMicrophoneSectionView: View {
    @AppStorage("microphone.inputDeviceID") private var selectedDeviceID = ""
    @StateObject private var testModel = MicrophoneTestModel()
    @State private var devices: [AudioInputDevice] = []

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: SettingsLayout.sectionSpacing) {
                SettingsGroupView(title: "Input Device") {
                    if devices.isEmpty {
                        Text("No input devices found.")
                            .foregroundStyle(.secondary)
                    } else {
                        Picker("Input device", selection: $selectedDeviceID) {
                            ForEach(devices) { device in
                                Text(device.name).tag(device.id)
                            }
                        }
                        .pickerStyle(.menu)
                    }
                }
                Divider()
                SettingsGroupView(title: "Input Level") {
                    VStack(alignment: .leading, spacing: SettingsLayout.inlineSpacing) {
                        SettingsLevelMeterView(level: testModel.level, isActive: testModel.isTesting)
                        if testModel.isTesting {
                            Text("Listening...")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        if let error = testModel.errorMessage {
                            Text(error)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
                Divider()
                SettingsGroupView(title: "Test Input") {
                    Button(testModel.isTesting ? "Stop" : "Test Input") {
                        testModel.toggleTest()
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                }
            }
            .padding(.vertical, SettingsLayout.scrollVerticalPadding)
        }
        .onAppear {
            devices = AudioInputDevice.available()
            if selectedDeviceID.isEmpty || !devices.contains(where: { $0.id == selectedDeviceID }) {
                selectedDeviceID = devices.first?.id ?? ""
            }
        }
    }
}
```

Create `apps/macos/InkFlow/InkFlow/UI/Settings/SettingsShortcutsSectionView.swift`:
```swift
import SwiftUI

struct SettingsShortcutsSectionView: View {
    @State private var toggleDictation = ""
    @State private var pushToTalk = ""
    @State private var pasteLastTranscript = ""

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: SettingsLayout.sectionSpacing) {
                SettingsGroupView(title: "Dictation") {
                    SettingsShortcutRow(title: "Toggle dictation", value: $toggleDictation)
                    SettingsShortcutRow(title: "Push-to-talk", value: $pushToTalk)
                }
                Divider()
                SettingsGroupView(title: "Output") {
                    SettingsShortcutRow(title: "Paste last transcript", value: $pasteLastTranscript)
                }
                Divider()
                SettingsGroupView(title: "Defaults") {
                    Button("Reset to defaults") {
                        toggleDictation = ""
                        pushToTalk = ""
                        pasteLastTranscript = ""
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                }
            }
            .padding(.vertical, SettingsLayout.scrollVerticalPadding)
        }
    }
}
```

**Step 5: Update `SettingsRootView`**

Modify `apps/macos/InkFlow/InkFlow/UI/Settings/SettingsRootView.swift`:
```swift
import SwiftUI

struct SettingsRootView: View {
    @AppStorage("settings.selectedSection") private var selectedSectionRaw = SettingsSection.appearance.rawValue
    @AppStorage("appearance.theme") private var themeRaw = ThemePreference.system.rawValue
    @AppStorage("appearance.accent") private var accentRaw = AccentOption.sky.rawValue
    @AppStorage("appearance.glassIntensity") private var glassIntensityRaw = GlassIntensity.standard.rawValue
    @AppStorage("appearance.windowTranslucency") private var isWindowTranslucent = true

    var body: some View {
        HStack(spacing: SettingsLayout.rootSpacing) {
            SettingsSidebarView(selectedSection: selectedSection) { selectedSectionRaw = $0.rawValue }
            Divider()
            detailView
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .preferredColorScheme(appearance.preferredColorScheme)
        .tint(appearance.accentColor)
    }

    private var selectedSection: SettingsSection {
        SettingsSection(rawValue: selectedSectionRaw) ?? .appearance
    }

    @ViewBuilder
    private var detailView: some View {
        Group {
            switch selectedSection {
            case .appearance:
                SettingsAppearanceSectionView()
            case .microphone:
                SettingsMicrophoneSectionView()
            case .shortcuts:
                SettingsShortcutsSectionView()
            }
        }
        .id(selectedSection)
        .transition(.opacity)
        .animation(.easeInOut(duration: UIDuration.standard), value: selectedSectionRaw)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }

    private var appearance: AppearanceStyle {
        AppearanceStyle(
            theme: AppearanceStyle.theme(from: themeRaw),
            accent: AppearanceStyle.accent(from: accentRaw),
            glassIntensity: AppearanceStyle.glassIntensity(from: glassIntensityRaw),
            isTranslucent: isWindowTranslucent
        )
    }
}
```

**Step 6: Skip commit**

Per user request, do not commit.

---

### Task 4: Performance improvements for orb and waveform

**Files:**
- Modify: `apps/macos/InkFlow/InkFlow/UI/Panel/Orb/ActivationOrbView.swift`
- Modify: `apps/macos/InkFlow/InkFlow/UI/Panel/Waveform/WaveformView.swift`

**Step 1: Add reusable buffers for orb rendering**

Update `ActivationOrbView.swift` by introducing a renderer type that stores reusable arrays:
```swift
private final class OrbRenderer {
    var transformedPoints: [TransformedPoint] = []
    var particles: [LetterParticle] = []

    func render(context: inout GraphicsContext, size: CGSize, isActive: Bool, startTime: TimeInterval) {
        // Move the existing math and drawing code here.
        // Reuse transformedPoints and particles arrays instead of allocating new ones each frame.
    }
}
```
Then update `LetterMorphOrbView` to keep a renderer instance:
```swift
@State private var renderer = OrbRenderer()
```
and call `renderer.render(...)` inside the `Canvas`.

**Step 2: Cache default waveform levels**

In `WaveformView.swift`, replace repeated array creation with a static cache:
```swift
private enum WaveformLayout {
    static let defaultBarCount: Int = 28
    static let defaultLevel: CGFloat = 0.04
    static let minimumLevel: CGFloat = 0.03
    static let barSpacing: CGFloat = 3
    static let minimumBarWidth: CGFloat = 2
    static let defaultLevels: [CGFloat] = Array(repeating: defaultLevel, count: defaultBarCount)
}
```
Then use `WaveformLayout.defaultLevels` when `levels` is empty.

**Step 3: Skip commit**

Per user request, do not commit.

---

### Task 5: Wire Xcode references and validate builds

**Files:**
- Modify: `apps/macos/InkFlow/InkFlow.xcodeproj/project.pbxproj`

**Step 1: Verify project references**

Ensure all moved/created files are included in the Xcode project groups and the build target’s Compile Sources phase.

**Step 2: Run a build (optional but recommended)**

Run:
```bash
xcodebuild -project apps/macos/InkFlow/InkFlow.xcodeproj -scheme InkFlow -configuration Debug -destination 'platform=macOS' build
```
Expected: `** BUILD SUCCEEDED **`.

**Step 3: Skip commit**

Per user request, do not commit.

