import SwiftUI

struct PanelBackgroundView: View {
	@ObservedObject var panelController: PanelController
	let appearance: AppearanceStyle

	var body: some View {
		GeometryReader { _ in
			let collapsedRadius = max(0, panelController.collapsedPanelHeight / 2)
			let cornerRadius =
				panelController.isExpanded ? UIPanelLayout.expandedCornerRadius : collapsedRadius
			let shape = RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
			Group {
				if #available(macOS 26.0, *), appearance.isTranslucent {
					GlassEffectContainer(spacing: UISpacing.medium) {
						Color.clear
							.frame(maxWidth: .infinity, maxHeight: .infinity)
							.glassEffect(
								.regular.tint(appearance.surfaceTint),
								in: .rect(cornerRadius: cornerRadius))
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
