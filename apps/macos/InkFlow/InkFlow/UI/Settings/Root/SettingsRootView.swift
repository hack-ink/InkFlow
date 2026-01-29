import SwiftUI

struct SettingsRootView: View {
	@AppStorage("settings.selectedSection") private var selectedSectionRaw = SettingsSection.appearance.rawValue
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

	private var selectedSection: SettingsSection {
		SettingsSection(rawValue: selectedSectionRaw) ?? .appearance
	}

	var body: some View {
		HStack(spacing: SettingsLayout.rootSpacing) {
			SettingsSidebarView(selectedSection: selectedSection) { selectedSectionRaw = $0.rawValue }
			Divider()
			detailView
		}
		.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
		.preferredColorScheme(appearance.preferredColorScheme)
		.tint(appearance.accentColor)
	}

	@ViewBuilder
	private var detailView: some View {
		Group {
			switch selectedSection {
			case .appearance:
				SettingsAppearanceSectionView()
			case .microphone:
				SettingsMicrophoneSectionView()
			case .shortcuts:
				SettingsShortcutsSectionView()
			}
		}
		.id(selectedSection)
		.transition(.opacity)
		.animation(.easeInOut(duration: UIDuration.standard), value: selectedSectionRaw)
		.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
	}
}
