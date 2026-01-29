import SwiftUI

struct MicrophoneLevelGroup: View {
	@ObservedObject var testModel: MicrophoneTestModel

	var body: some View {
		SettingsGroupView(title: "Input Level") {
			VStack(alignment: .leading, spacing: SettingsLayout.inlineSpacing) {
				SettingsLevelMeterView(level: testModel.level, isActive: testModel.isTesting)
				if testModel.isTesting {
					Text("Listening...")
						.font(.caption)
						.foregroundStyle(.secondary)
				}
				if let error = testModel.errorMessage {
					Text(error)
						.font(.caption)
						.foregroundStyle(.secondary)
				}
			}
		}
	}
}
