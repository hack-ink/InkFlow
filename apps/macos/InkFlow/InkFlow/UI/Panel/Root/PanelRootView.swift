import SwiftUI

struct PanelRootView: View {
	@ObservedObject var model: InkFlowViewModel
	@ObservedObject var panelController: PanelController
	@AppStorage("appearance.theme") private var themeRaw = ThemePreference.system.rawValue
	@AppStorage("appearance.accent") private var accentRaw = AccentOption.sky.rawValue
	@AppStorage("appearance.glassIntensity") private var glassIntensityRaw = GlassIntensity.standard.rawValue
	@AppStorage("appearance.windowTranslucency") private var isWindowTranslucent = true

	private var appearance: AppearanceStyle {
		AppearanceStyle(
			theme: AppearanceStyle.theme(from: themeRaw),
			accent: AppearanceStyle.accent(from: accentRaw),
			glassIntensity: AppearanceStyle.glassIntensity(from: glassIntensityRaw),
			isTranslucent: isWindowTranslucent
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
