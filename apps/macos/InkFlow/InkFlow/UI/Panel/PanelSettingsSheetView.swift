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
