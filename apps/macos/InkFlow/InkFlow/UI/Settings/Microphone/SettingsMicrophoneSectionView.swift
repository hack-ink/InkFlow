import SwiftUI

struct SettingsMicrophoneSectionView: View {
	@AppStorage("microphone.inputDeviceID") private var selectedDeviceID = ""
	@StateObject private var testModel = MicrophoneTestModel()
	@State private var devices: [AudioInputDevice] = []

	var body: some View {
		ScrollView {
			VStack(alignment: .leading, spacing: SettingsLayout.sectionSpacing) {
				MicrophoneInputDeviceGroup(
					devices: devices,
					selectedDeviceID: $selectedDeviceID
				)
				Divider()
				MicrophoneLevelGroup(testModel: testModel)
				Divider()
				MicrophoneTestGroup(testModel: testModel)
			}
			.padding(.vertical, SettingsLayout.scrollVerticalPadding)
		}
		.onAppear {
			refreshDevices()
		}
	}

	private func refreshDevices() {
		devices = AudioInputDevice.available()
		if selectedDeviceID.isEmpty || !devices.contains(where: { $0.id == selectedDeviceID }) {
			selectedDeviceID = devices.first?.id ?? ""
		}
	}
}
