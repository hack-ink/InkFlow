import AVFoundation
import Foundation

final class AudioCapture {
	private let engine = AVAudioEngine()
	private let queue = DispatchQueue(label: "inkflow.audio.capture", qos: .userInitiated)
	private var isRunning = false
	private var sampleRate: Double = 0
	private var channelCount: AVAudioChannelCount = 1

	func requestMicrophoneAccess(completion: @escaping (Bool) -> Void) {
		switch AVCaptureDevice.authorizationStatus(for: .audio) {
		case .authorized:
			completion(true)
		case .notDetermined:
			AVCaptureDevice.requestAccess(for: .audio) { granted in
				DispatchQueue.main.async {
					completion(granted)
				}
			}
		default:
			completion(false)
		}
	}

	func start(onBuffer: @escaping ([Float], Double) -> Void) throws {
		if isRunning {
			return
		}

		let input = engine.inputNode
		let inputFormat = input.outputFormat(forBus: 0)
		sampleRate = inputFormat.sampleRate
		channelCount = max(inputFormat.channelCount, 1)

		guard
			let format = AVAudioFormat(
				commonFormat: .pcmFormatFloat32,
				sampleRate: sampleRate,
				channels: 1,
				interleaved: false
			)
		else {
			throw AudioCaptureError.invalidFormat
		}

		input.installTap(onBus: 0, bufferSize: 1024, format: format) { [weak self] buffer, _ in
			guard let self else {
				return
			}
			guard let channelData = buffer.floatChannelData else {
				return
			}

			let frameCount = Int(buffer.frameLength)
			let samples = Array(UnsafeBufferPointer(start: channelData[0], count: frameCount))
			let sampleRate = format.sampleRate

			self.queue.async {
				onBuffer(samples, sampleRate)
			}
		}

		engine.prepare()
		try engine.start()
		isRunning = true
	}

	func stop() {
		if !isRunning {
			return
		}
		engine.inputNode.removeTap(onBus: 0)
		engine.stop()
		isRunning = false
	}
}

enum AudioCaptureError: Error {
	case invalidFormat
}
