import AppKit
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

struct PanelBackgroundView: View {
	@ObservedObject var panelController: PanelController
	@AppStorage("appearance.theme") private var themeRaw = ThemePreference.system.rawValue
	@AppStorage("appearance.accent") private var accentRaw = AccentOption.sky.rawValue
	@AppStorage("appearance.glassIntensity") private var glassIntensityRaw = GlassIntensity.standard.rawValue
	@AppStorage("appearance.windowTranslucency") private var isWindowTranslucent = true
	private let expandedCornerRadius: CGFloat = UIPanelLayout.expandedCornerRadius

	var body: some View {
		GeometryReader { proxy in
			let appearance = AppearanceStyle(
				theme: AppearanceStyle.theme(from: themeRaw),
				accent: AppearanceStyle.accent(from: accentRaw),
				glassIntensity: AppearanceStyle.glassIntensity(from: glassIntensityRaw),
				isTranslucent: isWindowTranslucent
			)
			let collapsedRadius = max(0, panelController.collapsedPanelHeight / 2)
			let cornerRadius = panelController.isExpanded ? expandedCornerRadius : collapsedRadius
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

struct PanelHeaderView: View {
	@ObservedObject var model: InkFlowViewModel
	@ObservedObject var panelController: PanelController
	@AppStorage("appearance.theme") private var themeRaw = ThemePreference.system.rawValue
	@AppStorage("appearance.accent") private var accentRaw = AccentOption.sky.rawValue
	@AppStorage("appearance.glassIntensity") private var glassIntensityRaw = GlassIntensity.standard.rawValue
	@AppStorage("appearance.windowTranslucency") private var isWindowTranslucent = true

	var body: some View {
		headerRow
			.preferredColorScheme(appearance.preferredColorScheme)
			.tint(appearance.accentColor)
			.onExitCommand {
				panelController.handleExitCommand()
			}
	}

	private var headerRow: some View {
		HStack(alignment: .center, spacing: HeaderLayout.rowSpacing) {
			leadingBlock
			transcriptStrip
			expandButton
		}
		.frame(maxWidth: .infinity, alignment: .leading)
		.frame(height: HeaderLayout.rowHeight, alignment: .center)
		.textSelection(.disabled)
	}

	private var leadingBlock: some View {
		HStack(spacing: HeaderLayout.leadingSpacing) {
			statusGlyph

			if let error = model.errorMessage {
				Text(error)
					.font(.system(size: 13, weight: .medium))
					.foregroundStyle(UIColors.errorText)
			}
		}
		.padding(.leading, HeaderLayout.leadingInset)
	}

	@ViewBuilder
	private var statusGlyph: some View {
		let button = Button(action: toggleListening) {
			ActivationOrbView(isActive: model.isListening)
		}
		.buttonStyle(.plain)
		.focusable(false)
		button
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
		.padding(.vertical, HeaderLayout.transcriptVerticalPadding)
		.padding(.horizontal, HeaderLayout.transcriptHorizontalPadding)
		.frame(height: HeaderLayout.transcriptHeight)
		.textSelection(.disabled)
		.allowsHitTesting(false)
	}

	private var expandButton: some View {
		Button(action: panelController.toggleExpanded) {
			Image(systemName: panelController.isExpanded ? "chevron.up" : "chevron.down")
				.font(.system(size: 11, weight: .semibold))
				.foregroundStyle(.secondary)
				.frame(width: HeaderLayout.expandButtonSize, height: HeaderLayout.expandButtonSize)
		}
		.buttonStyle(.plain)
		.focusable(false)
		.animation(.easeInOut(duration: UIDuration.standard), value: panelController.isExpanded)
		.accessibilityLabel(panelController.isExpanded ? "Collapse panel" : "Expand panel")
	}

	@ViewBuilder
	private var waveformBackdrop: some View {
		WaveformView(levels: model.waveformLevels, isActive: model.isListening)
			.opacity(model.isListening ? HeaderLayout.waveformActiveOpacity : HeaderLayout.waveformInactiveOpacity)
			.blur(radius: HeaderLayout.waveformBlurRadius)
			.frame(height: HeaderLayout.waveformHeight)
			.mask(waveformFadeMask)
	}

	private var waveformFadeMask: some View {
		LinearGradient(
			colors: [
				.clear,
				.white.opacity(HeaderLayout.waveformMaskOpacity),
				.white.opacity(HeaderLayout.waveformMaskOpacity),
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

	private var appearance: AppearanceStyle {
		AppearanceStyle(
			theme: AppearanceStyle.theme(from: themeRaw),
			accent: AppearanceStyle.accent(from: accentRaw),
			glassIntensity: AppearanceStyle.glassIntensity(from: glassIntensityRaw),
			isTranslucent: isWindowTranslucent
		)
	}
}

struct PanelExpandedView: View {
	@ObservedObject var panelController: PanelController
	@State private var selectedMode: PanelMode = .history
	@AppStorage("appearance.theme") private var themeRaw = ThemePreference.system.rawValue
	@AppStorage("appearance.accent") private var accentRaw = AccentOption.sky.rawValue
	@AppStorage("appearance.glassIntensity") private var glassIntensityRaw = GlassIntensity.standard.rawValue
	@AppStorage("appearance.windowTranslucency") private var isWindowTranslucent = true

	var body: some View {
		ZStack(alignment: .topTrailing) {
			VStack(alignment: .leading, spacing: ExpandedLayout.stackSpacing) {
				modeBar
				Divider().opacity(ExpandedLayout.dividerOpacity)
				moduleContent
			}
			.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)

			settingsAccessory

			if panelController.isSettingsPresented {
				settingsOverlay
					.transition(.opacity.combined(with: .scale(scale: 0.98)))
			}
		}
		.preferredColorScheme(appearance.preferredColorScheme)
		.tint(appearance.accentColor)
		.onExitCommand {
			panelController.handleExitCommand()
		}
		.animation(.easeInOut(duration: UIDuration.panelSettingsToggle), value: panelController.isSettingsPresented)
	}

	private var modeBar: some View {
		HStack(spacing: ExpandedLayout.modeBarSpacing) {
			ForEach(PanelMode.allCases) { mode in
				ModeTab(title: mode.title, isSelected: selectedMode == mode) {
					withAnimation(.easeInOut(duration: UIDuration.selectionChange)) {
						selectedMode = mode
					}
				}
			}
			Spacer(minLength: 0)
		}
		.frame(maxWidth: .infinity, alignment: .leading)
	}

	private var settingsAccessory: some View {
		Button(action: panelController.toggleSettings) {
			Image(systemName: "slider.horizontal.3")
				.font(.system(size: 12, weight: .semibold))
				.foregroundStyle(panelController.isSettingsPresented ? Color.primary : Color.secondary)
				.frame(width: ExpandedLayout.settingsButtonSize, height: ExpandedLayout.settingsButtonSize)
		}
		.buttonStyle(.plain)
		.accessibilityLabel("Settings")
		.padding(.top, ExpandedLayout.settingsButtonTopPadding)
	}

	private var moduleContent: some View {
		Color.clear
			.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
	}

	private var settingsOverlay: some View {
		ZStack {
			UIColors.overlayScrim
				.contentShape(Rectangle())
				.onTapGesture {
					panelController.closeSettings()
				}

			settingsSheet
		}
		.frame(maxWidth: .infinity, maxHeight: .infinity)
	}

	private var settingsSheet: some View {
		VStack(spacing: SettingsSheetLayout.stackSpacing) {
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

			SettingsView()
		}
		.padding(SettingsSheetLayout.padding)
		.frame(width: SettingsSheetLayout.width, height: SettingsSheetLayout.height, alignment: .topLeading)
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
			appearance.isTranslucent ? SettingsSheetLayout.translucentOpacity : 1.0
		)
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

private struct ModeTab: View {
	let title: String
	let isSelected: Bool
	let action: () -> Void

	var body: some View {
		Button(action: action) {
			VStack(spacing: ModeTabLayout.spacing) {
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

private enum HeaderLayout {
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

private enum ExpandedLayout {
	static let stackSpacing: CGFloat = UISpacing.medium
	static let modeBarSpacing: CGFloat = UISpacing.xLarge
	static let settingsButtonSize: CGFloat = UISize.iconMedium
	static let settingsButtonTopPadding: CGFloat = UISpacing.xxSmall
	static let dividerOpacity: Double = 0.35
}

private enum SettingsSheetLayout {
	static let width: CGFloat = 520
	static let height: CGFloat = 280
	static let padding: CGFloat = UISpacing.xLarge
	static let stackSpacing: CGFloat = UISpacing.large
	static let translucentOpacity: Double = 0.95
}

private enum ModeTabLayout {
	static let spacing: CGFloat = UISpacing.xSmall
}


#Preview {
	let panelController = PanelController()
	let model = InkFlowViewModel()
	ZStack {
		PanelBackgroundView(panelController: panelController)
		VStack(spacing: UIPanelLayout.headerSpacing) {
			PanelHeaderView(model: model, panelController: panelController)
			PanelExpandedView(panelController: panelController)
		}
		.padding(UIPanelLayout.padding)
	}
	.frame(width: UIPanelLayout.previewWidth, height: UIPanelLayout.previewHeight)
}
