import SwiftUI

struct AppearanceReader<Content: View>: View {
	let content: (AppearanceStyle) -> Content

	private var appearance: AppearanceStyle {
		let appearanceConfig = ConfigStore.shared.current.appearance
		AppearanceStyle(
			theme: AppearanceStyle.theme(from: appearanceConfig.theme),
			accent: AppearanceStyle.accent(from: appearanceConfig.accent),
			glassIntensity: AppearanceStyle.glassIntensity(from: appearanceConfig.glassIntensity),
			isTranslucent: appearanceConfig.isWindowTranslucent
		)
	}

	var body: some View {
		content(appearance)
	}
}
