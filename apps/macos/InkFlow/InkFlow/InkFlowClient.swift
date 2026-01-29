import Foundation

struct InkFlowUpdate: Decodable {
	let kind: String
	let text: String?
	let segmentId: UInt64?
	let code: String?
	let message: String?
	let result: InkFlowUpdateResult?

	enum CodingKeys: String, CodingKey {
		case kind
		case text
		case segmentId = "segment_id"
		case code
		case message
		case result
	}
}

struct InkFlowUpdateResult: Decodable {
	let text: String
}

final class InkFlowClient {
	private var handle: UnsafeMutablePointer<InkFlowHandle>?
	private var callbackRegistered = false
	private let decoder = JSONDecoder()
	private var onUpdate: ((InkFlowUpdate) -> Void)?

	init?() {
		guard let handle = inkflow_engine_create() else {
			return nil
		}
		self.handle = handle
	}

	deinit {
		stop()
	}

	func registerUpdates(_ onUpdate: @escaping (InkFlowUpdate) -> Void) -> Bool {
		guard let handle else {
			return false
		}

		self.onUpdate = onUpdate
		let userData = Unmanaged.passUnretained(self).toOpaque()
		let result = inkflow_engine_register_callback(handle, inkflowUpdateCallback, userData)
		callbackRegistered = result == INKFLOW_OK
		return callbackRegistered
	}

	func unregisterUpdates() {
		guard let handle else {
			return
		}
		if callbackRegistered {
			inkflow_engine_unregister_callback(handle)
			callbackRegistered = false
		}
		onUpdate = nil
	}

	func submitAudio(samples: [Float], sampleRate: Double) -> Bool {
		guard let handle else {
			return false
		}
		let rate = UInt32(sampleRate.rounded())
		let result = samples.withUnsafeBufferPointer { buffer in
			inkflow_engine_submit_audio(handle, buffer.baseAddress, buffer.count, rate)
		}
		return result == INKFLOW_OK
	}

	func stop() {
		unregisterUpdates()
		if let handle {
			inkflow_engine_destroy(handle)
			self.handle = nil
		}
	}

	fileprivate func handleCallback(_ cString: UnsafePointer<CChar>?) {
		guard let cString else {
			return
		}

		let jsonString = String(cString: cString)
		guard let data = jsonString.data(using: .utf8) else {
			return
		}

		do {
			let update = try decoder.decode(InkFlowUpdate.self, from: data)
			onUpdate?(update)
		} catch {
			return
		}
	}
}

private let inkflowUpdateCallback: inkflow_update_cb = { cString, userData in
	guard let userData else {
		return
	}
	let client = Unmanaged<InkFlowClient>.fromOpaque(userData).takeUnretainedValue()
	client.handleCallback(cString)
}
