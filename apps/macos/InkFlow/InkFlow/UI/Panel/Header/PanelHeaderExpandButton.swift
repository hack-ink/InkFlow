import SwiftUI

struct PanelHeaderExpandButton: View {
	@ObservedObject var panelController: PanelController

	var body: some View {
		Button(action: panelController.toggleExpanded) {
			Image(systemName: panelController.isExpanded ? "chevron.up" : "chevron.down")
				.font(.system(size: 11, weight: .semibold))
				.foregroundStyle(.secondary)
				.frame(
					width: PanelHeaderLayout.expandButtonSize,
					height: PanelHeaderLayout.expandButtonSize)
		}
		.buttonStyle(.plain)
		.focusable(false)
		.animation(.easeInOut(duration: UIDuration.standard), value: panelController.isExpanded)
		.accessibilityLabel(panelController.isExpanded ? "Collapse panel" : "Expand panel")
	}
}
