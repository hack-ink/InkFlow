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
