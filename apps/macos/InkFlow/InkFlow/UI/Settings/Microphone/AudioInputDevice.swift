import AVFoundation

struct AudioInputDevice: Identifiable {
	let id: String
	let name: String

	static func available() -> [AudioInputDevice] {
		let session = AVCaptureDevice.DiscoverySession(
			deviceTypes: [.microphone, .external],
			mediaType: .audio,
			position: .unspecified
		)
		return session.devices.map { AudioInputDevice(id: $0.uniqueID, name: $0.localizedName) }
	}
}
