import SwiftUI

struct SettingsAppearanceSectionView: View {
	@AppStorage("appearance.theme") private var themeRaw = ThemePreference.system.rawValue
	@AppStorage("appearance.accent") private var accentRaw = AccentOption.sky.rawValue
	@AppStorage("appearance.glassIntensity") private var glassIntensityRaw = GlassIntensity.standard.rawValue
	@AppStorage("appearance.windowTranslucency") private var isWindowTranslucent = true
	#if DEBUG
		@AppStorage("debug.showOrbFrame") private var showOrbFrame = false
	#endif

	private var themeBinding: Binding<ThemePreference> {
		Binding(
			get: { AppearanceStyle.theme(from: themeRaw) },
			set: { themeRaw = $0.rawValue }
		)
	}

	private var accentBinding: Binding<AccentOption> {
		Binding(
			get: { AppearanceStyle.accent(from: accentRaw) },
			set: { accentRaw = $0.rawValue }
		)
	}

	private var glassIntensityBinding: Binding<GlassIntensity> {
		Binding(
			get: { AppearanceStyle.glassIntensity(from: glassIntensityRaw) },
			set: { glassIntensityRaw = $0.rawValue }
		)
	}

	var body: some View {
		ScrollView {
			VStack(alignment: .leading, spacing: SettingsLayout.sectionSpacing) {
				AppearanceThemeGroup(theme: themeBinding)
				Divider()
				AppearanceAccentGroup(accent: accentBinding)
				Divider()
				AppearanceWindowGlassGroup(
					glassIntensity: glassIntensityBinding,
					isWindowTranslucent: $isWindowTranslucent
				)
				#if DEBUG
					Divider()
					AppearanceDebugGroup(showOrbFrame: $showOrbFrame)
				#endif
			}
			.padding(.vertical, SettingsLayout.scrollVerticalPadding)
		}
	}
}
