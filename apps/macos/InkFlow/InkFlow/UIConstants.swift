import SwiftUI

enum UISpacing {
	static let xxSmall: CGFloat = 2
	static let xSmall: CGFloat = 4
	static let small: CGFloat = 6
	static let medium: CGFloat = 8
	static let large: CGFloat = 12
	static let xLarge: CGFloat = 16
}

enum UISize {
	static let orbDiameter: CGFloat = 38
	static let iconSmall: CGFloat = 22
	static let iconMedium: CGFloat = 26
	static let transcriptHeight: CGFloat = 24
	static let waveformHeight: CGFloat = 16
	static let levelMeterHeight: CGFloat = 10
	static let accentSwatch: CGFloat = 20
	static let modeUnderlineHeight: CGFloat = 1
}

enum UICornerRadius {
	static let small: CGFloat = 8
	static let medium: CGFloat = 12
}

enum UIDuration {
	static let panelExpand: TimeInterval = 0.32
	static let panelSettingsToggle: TimeInterval = 0.22
	static let standard: TimeInterval = 0.2
	static let selectionChange: TimeInterval = 0.18
	static let hoverFade: TimeInterval = 0.15
	static let waveformLevel: TimeInterval = 0.08
	static let meterLevel: TimeInterval = 0.12
}

enum UIColors {
	static let errorText = Color.red.opacity(0.85)
	static let overlayScrim = Color.black.opacity(0.08)
	static let settingsBorder = Color.primary.opacity(0.06)
	static let settingsShadow = Color.black.opacity(0.12)
	static let sidebarSelectedBackground = Color.primary.opacity(0.1)
	static let sidebarHoverBackground = Color.primary.opacity(0.05)
	static let shortcutFieldBackground = Color.primary.opacity(0.06)
	static let levelMeterTrack = Color.primary.opacity(0.08)
	static let levelMeterActiveFill = Color.accentColor.opacity(0.7)
	static let levelMeterInactiveFill = Color.primary.opacity(0.3)
	static let selectionRing = Color.primary.opacity(0.8)
	static let waveformActiveTop = Color.primary.opacity(0.9)
	static let waveformActiveBottom = Color.primary.opacity(0.45)
	static let waveformInactiveTop = Color.primary.opacity(0.35)
	static let waveformInactiveBottom = Color.primary.opacity(0.18)
	static let modeTabIndicatorSelected = Color.primary.opacity(0.6)
	static let modeTabIndicatorUnselected = Color.primary.opacity(0.0)
}

enum UIShadow {
	static let settingsSheetRadius: CGFloat = 12
	static let settingsSheetX: CGFloat = 0
	static let settingsSheetY: CGFloat = 6
}

enum UIPanelLayout {
	static let padding: CGFloat = UISpacing.medium
	static let headerSpacing: CGFloat = UISpacing.small
	static let headerHeight: CGFloat = 40
	static let expandedCornerRadius: CGFloat = UICornerRadius.medium
	static let previewWidth: CGFloat = 720
	static let previewHeight: CGFloat = 360
}

enum UIOrbPalette {
	static let activeCore = Color(red: 0.96, green: 0.34, blue: 0.4)
	static let activeAccent = Color(red: 1.0, green: 0.6, blue: 0.66)
	static let activeHighlight = Color(red: 1.0, green: 0.7, blue: 0.74)
	static let inactiveCore = Color(red: 0.26, green: 0.64, blue: 0.92)
	static let inactiveAccent = Color(red: 0.52, green: 0.86, blue: 0.98)
	static let inactiveHighlight = Color(red: 0.6, green: 0.9, blue: 0.98)
}
