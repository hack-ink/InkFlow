import AVFoundation
import Combine
import CoreGraphics
import Foundation

@MainActor
final class InkFlowViewModel: ObservableObject {
	@Published var transcript: String = ""
	@Published var status: String = "Idle"
	@Published var isListening: Bool = false
	@Published var errorMessage: String?
	@Published var waveformLevels: [CGFloat] = Array(
		repeating: InkFlowViewModel.waveformFloor,
		count: InkFlowViewModel.waveformBarCount
	)

	private let audioCapture = AudioCapture()
	private let client: InkFlowClient?
	private var segments: [String] = []
	private var segmentIndex: [UInt64: Int] = [:]
	private var liveText: String = ""
	private var lastWaveformUpdate: TimeInterval = 0
	private var lastWaveformLevel: CGFloat = InkFlowViewModel.waveformFloor
	private let waveformUpdateInterval: TimeInterval = 0.02
	private static let waveformFloor: CGFloat = 0.04
	private static let waveformScale: CGFloat = 28.0
	private static let waveformPeakScale: CGFloat = 18.0
	private static let waveformSmoothing: CGFloat = 0.2
	private static let waveformBarCount = 28

	init(client: InkFlowClient? = nil) {
		if let client {
			self.client = client
			return
		}
		self.client = InkFlowClient()
	}

	func start() {
		if isListening {
			return
		}

		guard client != nil else {
			status = "Backend unavailable"
			errorMessage = "Failed to initialize the Rust engine."
			return
		}

		status = "Requesting microphone access"
		audioCapture.requestMicrophoneAccess { [weak self] granted in
			guard let self else {
				return
			}
			if !granted {
				self.status = "Microphone access denied"
				self.errorMessage = "Enable microphone access in System Settings."
				return
			}
			self.beginCapture()
		}
	}

	func stop() {
		if !isListening {
			return
		}
		audioCapture.stop()
		client?.unregisterUpdates()
		isListening = false
		status = "Stopped"
		resetWaveform()
	}

	func clear() {
		segments.removeAll()
		segmentIndex.removeAll()
		liveText = ""
		transcript = ""
		errorMessage = nil
		resetWaveform()
	}

	private func beginCapture() {
		guard let client else {
			status = "Backend unavailable"
			errorMessage = "Failed to initialize the Rust engine."
			return
		}

		guard
			client.registerUpdates({ [weak self] update in
				DispatchQueue.main.async {
					self?.handleUpdate(update)
				}
			})
		else {
			status = "Failed to start"
			errorMessage = "Could not register backend updates."
			return
		}

		do {
			try audioCapture.start { [weak self] samples, sampleRate in
				guard let self else {
					return
				}
				guard let client = self.client else {
					return
				}
				self.handleWaveformInput(samples)
				let ok = client.submitAudio(samples: samples, sampleRate: sampleRate)
				if !ok {
					DispatchQueue.main.async {
						self.status = "Audio submit failed"
					}
				}
			}
			isListening = true
			status = "Listening"
			errorMessage = nil
			resetWaveform()
		} catch {
			status = "Audio capture failed"
			errorMessage = "Unable to start audio engine."
			client.unregisterUpdates()
		}
	}

	private func handleUpdate(_ update: InkFlowUpdate) {
		switch update.kind {
		case "sherpa_partial":
			liveText = update.text ?? ""
		case "window_result":
			liveText = update.result?.text ?? update.text ?? liveText
		case "segment_end":
			let text = update.text ?? ""
			let segmentId = update.segmentId ?? UInt64(segments.count + 1)
			segmentIndex[segmentId] = segments.count
			segments.append(text)
			liveText = ""
		case "second_pass":
			let text = update.text ?? ""
			if let segmentId = update.segmentId, let index = segmentIndex[segmentId] {
				if index < segments.count {
					segments[index] = text
				}
			}
		case "endpoint_reset":
			liveText = ""
		case "error":
			status = "Backend error"
			errorMessage = update.message ?? "An unknown error occurred."
		default:
			break
		}

		updateTranscript()
	}

	private func updateTranscript() {
		let committed = segments.joined(separator: " ").trimmingCharacters(in: .whitespacesAndNewlines)
		let live = liveText.trimmingCharacters(in: .whitespacesAndNewlines)
		if committed.isEmpty {
			transcript = live
		} else if live.isEmpty {
			transcript = committed
		} else {
			transcript = committed + " " + live
		}
	}

	private func handleWaveformInput(_ samples: [Float]) {
		let level = Self.computeWaveformLevel(samples)
		DispatchQueue.main.async { [weak self] in
			self?.applyWaveformLevel(level)
		}
	}

	private static func computeWaveformLevel(_ samples: [Float]) -> CGFloat {
		guard !samples.isEmpty else {
			return waveformFloor
		}
		var sum: Float = 0
		var peak: Float = 0
		for sample in samples {
			let magnitude = abs(sample)
			if magnitude > peak {
				peak = magnitude
			}
			sum += sample * sample
		}
		let mean = sum / Float(samples.count)
		let rms = sqrt(mean)
		let scaledRms = CGFloat(rms) * waveformScale
		let scaledPeak = CGFloat(peak) * waveformPeakScale
		let combined = max(scaledRms, scaledPeak)
		return min(max(combined, waveformFloor), 1.0)
	}

	private func applyWaveformLevel(_ level: CGFloat) {
		let now = CFAbsoluteTimeGetCurrent()
		if now - lastWaveformUpdate < waveformUpdateInterval {
			return
		}
		lastWaveformUpdate = now
		let smoothing = Self.waveformSmoothing
		let smoothed = (level + lastWaveformLevel * smoothing) / (1.0 + smoothing)
		lastWaveformLevel = smoothed
		pushWaveformLevel(smoothed)
	}

	private func pushWaveformLevel(_ level: CGFloat) {
		guard !waveformLevels.isEmpty else {
			return
		}
		var updated = waveformLevels
		updated.removeFirst()
		updated.append(level)
		waveformLevels = updated
	}

	private func resetWaveform() {
		waveformLevels = Array(repeating: Self.waveformFloor, count: waveformLevels.count)
		lastWaveformLevel = Self.waveformFloor
		lastWaveformUpdate = 0
	}

}
