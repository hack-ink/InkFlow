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
