import SwiftUI

struct PanelExpandedView: View {
	@ObservedObject var panelController: PanelController
	let appearance: AppearanceStyle

	var body: some View {
		Color.clear
			.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
			.preferredColorScheme(appearance.preferredColorScheme)
			.tint(appearance.accentColor)
			.onExitCommand { panelController.handleExitCommand() }
	}
}
