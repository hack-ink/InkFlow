import AVFoundation
import Combine
import SwiftUI

final class MicrophoneTestModel: ObservableObject {
	@Published var level: CGFloat = 0
	@Published var isTesting = false
	@Published var errorMessage: String?

	private let engine = AVAudioEngine()
	private let queue = DispatchQueue(label: "inkflow.microphone.test", qos: .userInitiated)
	private var lastUpdate: TimeInterval = 0
	private var lastLevel: CGFloat = 0
	private var stopWorkItem: DispatchWorkItem?

	func toggleTest() {
		if isTesting {
			stop()
		} else {
			start()
		}
	}

	private func start() {
		guard !isTesting else {
			return
		}

		errorMessage = nil
		requestMicrophoneAccess { [weak self] granted in
			guard let self else {
				return
			}
			guard granted else {
				self.errorMessage = "Enable microphone access in System Settings."
				return
			}
			self.beginCapture()
		}
	}

	private func beginCapture() {
		queue.async { [weak self] in
			guard let self else {
				return
			}

			do {
				try self.configureEngine()
				try self.engine.start()
				DispatchQueue.main.async {
					self.isTesting = true
				}
				self.scheduleAutoStop()
			} catch {
				DispatchQueue.main.async {
					self.errorMessage = "Microphone test failed."
					self.isTesting = false
				}
				self.engine.stop()
			}
		}
	}

	private func configureEngine() throws {
		engine.stop()
		engine.inputNode.removeTap(onBus: 0)

		let format = engine.inputNode.outputFormat(forBus: 0)
		engine.inputNode.installTap(onBus: 0, bufferSize: 512, format: format) { [weak self] buffer, _ in
			guard let self else {
				return
			}
			let level = self.calculateLevel(from: buffer)
			self.updateLevel(level)
		}
	}

	private func calculateLevel(from buffer: AVAudioPCMBuffer) -> CGFloat {
		guard let channelData = buffer.floatChannelData?.pointee else {
			return 0
		}
		let frameLength = Int(buffer.frameLength)
		if frameLength == 0 {
			return 0
		}
		var sum: Float = 0
		for index in 0..<frameLength {
			let sample = channelData[index]
			sum += sample * sample
		}
		let rms = sqrt(sum / Float(frameLength))
		let scaled = min(max(rms * 8, 0), 1)
		return CGFloat(scaled)
	}

	private func updateLevel(_ level: CGFloat) {
		let now = CACurrentMediaTime()
		if now - lastUpdate < 0.05 {
			lastLevel = level
			return
		}
		lastUpdate = now
		let averaged = (lastLevel + level) / 2
		lastLevel = level
		DispatchQueue.main.async {
			self.level = averaged
		}
	}

	private func scheduleAutoStop() {
		stopWorkItem?.cancel()
		let workItem = DispatchWorkItem { [weak self] in
			self?.stop()
		}
		stopWorkItem = workItem
		DispatchQueue.main.asyncAfter(deadline: .now() + 6, execute: workItem)
	}

	private func stop() {
		stopWorkItem?.cancel()
		stopWorkItem = nil
		engine.stop()
		engine.inputNode.removeTap(onBus: 0)
		DispatchQueue.main.async {
			self.isTesting = false
			self.level = 0
		}
	}

	private func requestMicrophoneAccess(_ completion: @escaping (Bool) -> Void) {
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
}
