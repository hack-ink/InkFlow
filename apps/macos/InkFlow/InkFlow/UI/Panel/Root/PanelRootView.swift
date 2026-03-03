import SwiftUI

struct PanelRootView: View {
	@ObservedObject var model: InkFlowViewModel
	@ObservedObject var panelController: PanelController

	private var appearance: AppearanceStyle {
		let appearanceConfig = ConfigStore.shared.current.appearance
		return AppearanceStyle(
			theme: AppearanceStyle.theme(from: appearanceConfig.theme),
			accent: AppearanceStyle.accent(from: appearanceConfig.accent),
			glassIntensity: AppearanceStyle.glassIntensity(from: appearanceConfig.glassIntensity),
			isTranslucent: appearanceConfig.isWindowTranslucent
		)
	}

	var body: some View {
		ZStack {
			PanelBackgroundView(panelController: panelController, appearance: appearance)
			VStack(spacing: UIPanelLayout.headerSpacing) {
				PanelHeaderView(model: model, panelController: panelController, appearance: appearance)
				PanelExpandedView(panelController: panelController, appearance: appearance)
			}
			.padding(UIPanelLayout.padding)
		}
	}
}

#Preview {
	let panelController = PanelController()
	let model = InkFlowViewModel()
	PanelRootView(model: model, panelController: panelController)
		.frame(width: UIPanelLayout.previewWidth, height: UIPanelLayout.previewHeight)
}
