import SwiftUI

struct PanelHeaderStatusView: View {
	@ObservedObject var model: InkFlowViewModel
	let onToggle: () -> Void

	var body: some View {
		HStack(spacing: PanelHeaderLayout.leadingSpacing) {
			Button(action: onToggle) {
				ActivationOrbView(isActive: model.isListening)
			}
			.buttonStyle(.plain)
			.focusable(false)

			if let error = model.errorMessage {
				Text(error)
					.font(.system(size: 13, weight: .medium))
					.foregroundStyle(UIColors.errorText)
			}
		}
		.padding(.leading, PanelHeaderLayout.leadingInset)
	}
}
