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

	private var glassIntensityBinding: Binding<GlassIntensity> {
		Binding(
			get: { AppearanceStyle.glassIntensity(from: glassIntensityRaw) },
			set: { glassIntensityRaw = $0.rawValue }
		)
	}

	var body: some View {
		ScrollView {
			VStack(alignment: .leading, spacing: SettingsLayout.sectionSpacing) {
				SettingsGroupView(title: "Theme") {
					Picker("Theme", selection: themeBinding) {
						ForEach(ThemePreference.allCases) { option in
							Text(option.title)
								.tag(option)
						}
					}
					.pickerStyle(.segmented)
				}
				Divider()
				SettingsGroupView(title: "Accent Color") {
					LazyVGrid(
						columns: Array(
							repeating: GridItem(
								.fixed(SettingsLayout.accentGridItemSize),
								spacing: SettingsLayout.accentGridSpacing),
							count: SettingsLayout.accentGridColumns
						),
						spacing: SettingsLayout.accentGridSpacing
					) {
						ForEach(AccentOption.allCases) { option in
							Button {
								withAnimation(
									.easeInOut(duration: UIDuration.selectionChange)
								) {
									accentRaw = option.rawValue
								}
							} label: {
								Circle()
									.fill(option.color)
									.frame(
										width: UISize.accentSwatch,
										height: UISize.accentSwatch
									)
									.overlay(selectionRing(for: option))
							}
							.buttonStyle(.plain)
							.accessibilityLabel(option.title)
						}
					}
				}
				Divider()
				SettingsGroupView(title: "Window and Glass") {
					VStack(alignment: .leading, spacing: SettingsLayout.groupSpacing) {
						Picker("Glass intensity", selection: glassIntensityBinding) {
							ForEach(GlassIntensity.allCases) { option in
								Text(option.title)
									.tag(option)
							}
						}
						.pickerStyle(.segmented)

						Toggle("Window translucency", isOn: $isWindowTranslucent)
					}
				}
				#if DEBUG
					Divider()
					SettingsGroupView(title: "Debug") {
						Toggle("Show orb frame", isOn: $showOrbFrame)
					}
				#endif
			}
			.padding(.vertical, SettingsLayout.scrollVerticalPadding)
		}
	}

	@ViewBuilder
	private func selectionRing(for option: AccentOption) -> some View {
		if AppearanceStyle.accent(from: accentRaw) == option {
			Circle()
				.strokeBorder(UIColors.selectionRing, lineWidth: SettingsLayout.selectionRingLineWidth)
		}
	}
}
