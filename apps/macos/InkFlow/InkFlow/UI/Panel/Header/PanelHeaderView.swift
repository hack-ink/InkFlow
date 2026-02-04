import SwiftUI

struct PanelHeaderView: View {
	@ObservedObject var model: InkFlowViewModel
	@ObservedObject var panelController: PanelController
	let appearance: AppearanceStyle

	var body: some View {
		HStack(alignment: .center, spacing: PanelHeaderLayout.rowSpacing) {
			PanelHeaderStatusView(model: model, onToggle: toggleListening)
			PanelHeaderTranscriptView(model: model)
			PanelHeaderExpandButton(panelController: panelController)
		}
		.frame(maxWidth: .infinity, alignment: .leading)
		.frame(height: PanelHeaderLayout.rowHeight, alignment: .center)
		.textSelection(.disabled)
		.preferredColorScheme(appearance.preferredColorScheme)
		.tint(appearance.accentColor)
		.onExitCommand { panelController.handleExitCommand() }
	}

	private func toggleListening() {
		if model.isListening {
			model.stop()
		} else {
			model.start()
		}
	}
}
