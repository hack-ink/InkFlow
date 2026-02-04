import AppKit
import Foundation

struct HotkeySpec {
	let keyCode: UInt16
	let modifiers: NSEvent.ModifierFlags

	func matches(_ event: NSEvent) -> Bool {
		if keyCode == KeyCodeMap.functionKeyCode {
			return matchesFunctionKey(event)
		}
		let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
		return event.keyCode == keyCode && flags == modifiers
	}

	static func parse(from raw: String) -> HotkeySpec? {
		let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
		if let keyCode = UInt16(trimmed) {
			return HotkeySpec(keyCode: keyCode, modifiers: [])
		}

		let parts = trimmed.lowercased().split(separator: "+").map { $0.trimmingCharacters(in: .whitespaces) }
		if parts.isEmpty {
			return nil
		}

		var modifiers: NSEvent.ModifierFlags = []
		var keyToken: String?

		for part in parts {
			switch part {
			case "cmd", "command":
				modifiers.insert(.command)
			case "shift":
				modifiers.insert(.shift)
			case "option", "alt":
				modifiers.insert(.option)
			case "control", "ctrl":
				modifiers.insert(.control)
			default:
				keyToken = String(part)
			}
		}

		guard let keyToken, let keyCode = KeyCodeMap.keyCode(for: keyToken) else {
			return nil
		}

		return HotkeySpec(keyCode: keyCode, modifiers: modifiers)
	}

	private func matchesFunctionKey(_ event: NSEvent) -> Bool {
		guard event.keyCode == KeyCodeMap.functionKeyCode else {
			return false
		}
		let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
		return flags.contains(.function) || flags.isEmpty
	}
}

enum KeyCodeMap {
	private static let keyCodes: [String: UInt16] = [
		"fn": 63,
		"globe": 63,
		"function": 63,
		"space": 49,
		"return": 36,
		"enter": 36,
		"escape": 53,
		"esc": 53,
		"tab": 48,
		"a": 0,
		"s": 1,
		"d": 2,
		"f": 3,
		"h": 4,
		"g": 5,
		"z": 6,
		"x": 7,
		"c": 8,
		"v": 9,
		"b": 11,
		"q": 12,
		"w": 13,
		"e": 14,
		"r": 15,
		"y": 16,
		"t": 17,
		"1": 18,
		"2": 19,
		"3": 20,
		"4": 21,
		"6": 22,
		"5": 23,
		"9": 25,
		"7": 26,
		"8": 28,
		"0": 29,
		"o": 31,
		"u": 32,
		"i": 34,
		"p": 35,
		"l": 37,
		"j": 38,
		"k": 40,
		"n": 45,
		"m": 46,
	]

	static func keyCode(for token: String) -> UInt16? {
		keyCodes[token]
	}

	static let functionKeyCode: UInt16 = 63
}

final class DictationHotkeyController {
	private let viewModel: InkFlowViewModel
	private let panelController: PanelController
	private let config: AppConfig.Dictation
	private let hotkey: HotkeySpec
	private var localMonitor: Any?
	private var globalMonitor: Any?

	init?(viewModel: InkFlowViewModel, panelController: PanelController, config: AppConfig.Dictation) {
		self.viewModel = viewModel
		self.panelController = panelController
		self.config = config
		guard let hotkey = HotkeySpec.parse(from: config.hotkey) else {
			NSLog("Failed to parse dictation hotkey: %@.", config.hotkey)
			return nil
		}
		self.hotkey = hotkey
	}

	func start() {
		localMonitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .keyUp]) { [weak self] event in
			self?.handle(event)
			return event
		}
		globalMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.keyDown, .keyUp]) { [weak self] event in
			self?.handle(event)
		}
	}

	func stop() {
		if let monitor = localMonitor {
			NSEvent.removeMonitor(monitor)
			localMonitor = nil
		}
		if let monitor = globalMonitor {
			NSEvent.removeMonitor(monitor)
			globalMonitor = nil
		}
	}

	private func handle(_ event: NSEvent) {
		guard hotkey.matches(event) else {
			return
		}

		switch config.activationMode {
		case .hold:
			if event.type == .keyDown, !event.isARepeat {
				startDictation()
			} else if event.type == .keyUp {
				stopDictation()
			}
		case .toggle:
			if event.type == .keyDown, !event.isARepeat {
				toggleDictation()
			}
		}
	}

	private func startDictation() {
		Task { @MainActor in
			guard !viewModel.isListening else {
				return
			}
			panelController.showPanel()
			viewModel.start()
		}
	}

	private func stopDictation() {
		Task { @MainActor in
			guard viewModel.isListening else {
				return
			}
			viewModel.stop()
		}
	}

	private func toggleDictation() {
		Task { @MainActor in
			if viewModel.isListening {
				viewModel.stop()
			} else {
				panelController.showPanel()
				viewModel.start()
			}
		}
	}
}
