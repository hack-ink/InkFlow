import SwiftUI

struct AppearanceThemeGroup: View {
	@Binding var theme: ThemePreference

	var body: some View {
		SettingsGroupView(title: "Theme") {
			Picker("Theme", selection: $theme) {
				ForEach(ThemePreference.allCases) { option in
					Text(option.title)
						.tag(option)
				}
			}
			.pickerStyle(.segmented)
		}
	}
}
