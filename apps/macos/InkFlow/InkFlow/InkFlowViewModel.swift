import AVFoundation
import Combine
import Foundation

@MainActor
final class InkFlowViewModel: ObservableObject {
	@Published var transcript: String = ""
	@Published var status: String = "Idle"
	@Published var isListening: Bool = false
	@Published var errorMessage: String?

	private let audioCapture = AudioCapture()
	private let client: InkFlowClient?
	private var segments: [String] = []
	private var segmentIndex: [UInt64: Int] = [:]
	private var liveText: String = ""

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
	}

	func clear() {
		segments.removeAll()
		segmentIndex.removeAll()
		liveText = ""
		transcript = ""
		errorMessage = nil
	}

	private func beginCapture() {
		guard let client else {
			status = "Backend unavailable"
			errorMessage = "Failed to initialize the Rust engine."
			return
		}

		guard client.registerUpdates({ [weak self] update in
			DispatchQueue.main.async {
				self?.handleUpdate(update)
			}
		}) else {
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

}
