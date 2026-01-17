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

		let shape = RoundedRectangle(cornerRadius: 12, style: .continuous)
		Group {
			if #available(macOS 26.0, *), appearance.isTranslucent {
				GlassEffectContainer(spacing: 8) {
					Color.clear
						.frame(maxWidth: .infinity, maxHeight: .infinity)
						.glassEffect(.regular.tint(appearance.surfaceTint), in: .rect(cornerRadius: 12))
				}
			} else if appearance.isTranslucent {
				shape.fill(.ultraThinMaterial)
			} else {
				shape.fill(Color(nsColor: .windowBackgroundColor))
			}
		}
		.frame(maxWidth: .infinity, maxHeight: .infinity)
		.allowsHitTesting(false)
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
		HStack(alignment: .center, spacing: 9) {
			leadingBlock
			transcriptStrip
			expandButton
		}
		.frame(maxWidth: .infinity, alignment: .leading)
		.textSelection(.disabled)
	}

	private var leadingBlock: some View {
		HStack(spacing: 8) {
			statusGlyph

			if let error = model.errorMessage {
				Text(error)
					.font(.system(size: 13, weight: .medium))
					.foregroundStyle(.red.opacity(0.85))
			}
		}
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
		.padding(.vertical, 2)
		.padding(.horizontal, 4)
		.frame(height: 24)
		.textSelection(.disabled)
		.allowsHitTesting(false)
	}

	private var expandButton: some View {
		Button(action: panelController.toggleExpanded) {
			Image(systemName: panelController.isExpanded ? "chevron.up" : "chevron.down")
				.font(.system(size: 11, weight: .semibold))
				.foregroundStyle(.secondary)
				.frame(width: 22, height: 22)
		}
		.buttonStyle(.plain)
		.focusable(false)
		.animation(.easeInOut(duration: 0.2), value: panelController.isExpanded)
		.accessibilityLabel(panelController.isExpanded ? "Collapse panel" : "Expand panel")
	}

	@ViewBuilder
	private var waveformBackdrop: some View {
		WaveformView(levels: model.waveformLevels, isActive: model.isListening)
			.opacity(model.isListening ? 0.12 : 0.05)
			.blur(radius: 0.4)
			.frame(height: 16)
			.mask(waveformFadeMask)
	}

	private var waveformFadeMask: some View {
		LinearGradient(
			colors: [.clear, .white.opacity(0.9), .white.opacity(0.9), .clear],
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
			VStack(alignment: .leading, spacing: 8) {
				modeBar
				Divider().opacity(0.35)
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
		.animation(.easeInOut(duration: 0.22), value: panelController.isSettingsPresented)
	}

	private var modeBar: some View {
		HStack(spacing: 16) {
			ForEach(PanelMode.allCases) { mode in
				ModeTab(title: mode.title, isSelected: selectedMode == mode) {
					withAnimation(.easeInOut(duration: 0.18)) {
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
				.frame(width: 26, height: 26)
		}
		.buttonStyle(.plain)
		.accessibilityLabel("Settings")
		.padding(.top, 2)
	}

	private var moduleContent: some View {
		Color.clear
			.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
	}

	private var settingsOverlay: some View {
		ZStack {
			Color.black.opacity(0.08)
				.contentShape(Rectangle())
				.onTapGesture {
					panelController.closeSettings()
				}

			settingsSheet
		}
		.frame(maxWidth: .infinity, maxHeight: .infinity)
	}

	private var settingsSheet: some View {
		VStack(spacing: 12) {
			HStack {
				Text("Settings")
					.font(.system(size: 13, weight: .semibold))
					.foregroundStyle(.primary)
				Spacer()
				Button(action: panelController.closeSettings) {
					Image(systemName: "xmark")
						.font(.system(size: 11, weight: .semibold))
						.foregroundStyle(.secondary)
						.frame(width: 22, height: 22)
				}
				.buttonStyle(.plain)
				.accessibilityLabel("Close settings")
			}

			SettingsView()
		}
		.padding(16)
		.frame(width: 520, height: 280, alignment: .topLeading)
		.background(settingsSheetBackground)
		.clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
		.overlay(RoundedRectangle(cornerRadius: 12, style: .continuous)
			.strokeBorder(Color.primary.opacity(0.06)))
		.shadow(color: .black.opacity(0.12), radius: 12, x: 0, y: 6)
	}

	private var settingsSheetBackground: some View {
		Color(nsColor: .windowBackgroundColor).opacity(appearance.isTranslucent ? 0.95 : 1.0)
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
			VStack(spacing: 4) {
				Text(title)
					.font(.system(size: 12, weight: .semibold))
					.foregroundStyle(isSelected ? Color.primary : Color.secondary)
					.lineLimit(1)
					.truncationMode(.tail)
				Rectangle()
					.frame(height: 1)
					.foregroundStyle(Color.primary.opacity(isSelected ? 0.6 : 0))
			}
		}
		.buttonStyle(.plain)
		.animation(.easeInOut(duration: 0.18), value: isSelected)
	}
}

#Preview {
	let panelController = PanelController()
	let model = InkFlowViewModel()
	ZStack {
		PanelBackgroundView()
		VStack(spacing: 6) {
			PanelHeaderView(model: model, panelController: panelController)
			PanelExpandedView(panelController: panelController)
		}
		.padding(8)
	}
	.frame(width: 720, height: 360)
}
