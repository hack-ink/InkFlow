import SwiftUI

struct AppearanceWindowGlassGroup: View {
	@Binding var glassIntensity: GlassIntensity
	@Binding var isWindowTranslucent: Bool

	var body: some View {
		SettingsGroupView(title: "Window and Glass") {
			VStack(alignment: .leading, spacing: SettingsLayout.groupSpacing) {
				Picker("Glass intensity", selection: $glassIntensity) {
					ForEach(GlassIntensity.allCases) { option in
						Text(option.title)
							.tag(option)
					}
				}
				.pickerStyle(.segmented)

				Toggle("Window translucency", isOn: $isWindowTranslucent)
			}
		}
	}
}
