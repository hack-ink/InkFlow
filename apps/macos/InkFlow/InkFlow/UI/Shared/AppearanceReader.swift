import SwiftUI

struct AppearanceReader<Content: View>: View {
	let content: (AppearanceStyle) -> Content
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
		content(appearance)
	}
}
