import SwiftUI

struct MicrophoneTestGroup: View {
	@ObservedObject var testModel: MicrophoneTestModel

	var body: some View {
		SettingsGroupView(title: "Test Input") {
			Button(testModel.isTesting ? "Stop" : "Test Input") {
				testModel.toggleTest()
			}
			.buttonStyle(.bordered)
			.controlSize(.small)
		}
	}
}
