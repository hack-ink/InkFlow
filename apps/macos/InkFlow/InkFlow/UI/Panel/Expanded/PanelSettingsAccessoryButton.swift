import SwiftUI

struct PanelSettingsAccessoryButton: View {
	let isPresented: Bool
	let onToggle: () -> Void

	var body: some View {
		Button(action: onToggle) {
			Image(systemName: "slider.horizontal.3")
				.font(.system(size: 12, weight: .semibold))
				.foregroundStyle(isPresented ? Color.primary : Color.secondary)
				.frame(
					width: PanelExpandedLayout.settingsButtonSize,
					height: PanelExpandedLayout.settingsButtonSize)
		}
		.buttonStyle(.plain)
		.accessibilityLabel("Settings")
	}
}
