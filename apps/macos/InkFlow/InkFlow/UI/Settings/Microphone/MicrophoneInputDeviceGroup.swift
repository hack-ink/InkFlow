import SwiftUI

struct MicrophoneInputDeviceGroup: View {
	let devices: [AudioInputDevice]
	@Binding var selectedDeviceID: String

	var body: some View {
		SettingsGroupView(title: "Input Device") {
			if devices.isEmpty {
				Text("No input devices found.")
					.foregroundStyle(.secondary)
			} else {
				Picker("Input device", selection: $selectedDeviceID) {
					ForEach(devices) { device in
						Text(device.name)
							.tag(device.id)
					}
				}
				.pickerStyle(.menu)
			}
		}
	}
}
