import SwiftUI

struct AppearanceDebugGroup: View {
	@Binding var showOrbFrame: Bool

	var body: some View {
		SettingsGroupView(title: "Debug") {
			Toggle("Show orb frame", isOn: $showOrbFrame)
		}
	}
}
