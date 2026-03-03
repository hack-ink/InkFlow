import Foundation

struct AppConfig {
	struct Appearance {
		var theme: String
		var accent: String
		var glassIntensity: String
		var isWindowTranslucent: Bool
	}

	struct Dictation {
		var activationMode: ActivationMode
		var hotkey: String
	}

	enum ActivationMode: String {
		case hold
		case toggle
	}

	var appearance: Appearance
	var dictation: Dictation

	static let `default` = AppConfig(
		appearance: Appearance(
			theme: "system",
			accent: "sky",
			glassIntensity: "standard",
			isWindowTranslucent: true
		),
		dictation: Dictation(
			activationMode: .hold,
			hotkey: "fn"
		)
	)

	static let defaultToml = """
		[appearance]
		theme = "system"
		accent = "sky"
		glass_intensity = "standard"
		window_translucency = true

		[dictation]
		activation_mode = "hold"
		hotkey = "fn"
		"""

	init(appearance: Appearance, dictation: Dictation) {
		self.appearance = appearance
		self.dictation = dictation
	}

	init(parsed: TomlDictionary) {
		let appearance = Self.parseAppearance(from: parsed)
		let dictation = Self.parseDictation(from: parsed)
		self.appearance = appearance
		self.dictation = dictation
	}

	private static func parseAppearance(from parsed: TomlDictionary) -> Appearance {
		Appearance(
			theme: parsed.stringValue(for: "appearance.theme") ?? Self.default.appearance.theme,
			accent: parsed.stringValue(for: "appearance.accent") ?? Self.default.appearance.accent,
			glassIntensity: parsed.stringValue(for: "appearance.glass_intensity")
				?? Self.default.appearance.glassIntensity,
			isWindowTranslucent: parsed.boolValue(for: "appearance.window_translucency")
				?? Self.default.appearance.isWindowTranslucent
		)
	}

	private static func parseDictation(from parsed: TomlDictionary) -> Dictation {
		let rawMode = parsed.stringValue(for: "dictation.activation_mode")
		let mode = ActivationMode(rawValue: rawMode ?? "") ?? Self.default.dictation.activationMode
		let hotkey = parsed.stringValue(for: "dictation.hotkey") ?? Self.default.dictation.hotkey
		return Dictation(activationMode: mode, hotkey: hotkey)
	}
}

final class ConfigStore {
	static let shared = ConfigStore()

	private(set) var current: AppConfig

	private init() {
		current = AppConfig.default
		let url = configURL()
		ensureConfigFileExists(at: url)
		loadConfig(from: url)
	}

	private func loadConfig(from url: URL) {
		do {
			let content = try String(contentsOf: url, encoding: .utf8)
			let parsed = TomlParser.parse(content)
			current = AppConfig(parsed: parsed)
		} catch {
			NSLog("Failed to read config file at %@. %@", url.path, error.localizedDescription)
		}
	}

	private func ensureConfigFileExists(at url: URL) {
		let fileManager = FileManager.default
		if fileManager.fileExists(atPath: url.path) {
			return
		}
		do {
			let directory = url.deletingLastPathComponent()
			try fileManager.createDirectory(at: directory, withIntermediateDirectories: true)
			try AppConfig.defaultToml.write(to: url, atomically: true, encoding: .utf8)
		} catch {
			NSLog("Failed to create default config file at %@. %@", url.path, error.localizedDescription)
		}
	}

	private func configURL() -> URL {
		let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)
		let directory = base.first?.appendingPathComponent("InkFlow", isDirectory: true)
		return (directory ?? URL(fileURLWithPath: "/tmp")).appendingPathComponent("config.toml")
	}
}

struct TomlDictionary {
	private var values: [String: TomlValue]

	init(values: [String: TomlValue]) {
		self.values = values
	}

	func stringValue(for key: String) -> String? {
		switch values[key] {
		case .string(let value):
			return value
		case .int(let value):
			return String(value)
		case .bool(let value):
			return String(value)
		case .none:
			return nil
		}
	}

	func boolValue(for key: String) -> Bool? {
		switch values[key] {
		case .bool(let value):
			return value
		case .string(let value):
			return Bool(value)
		default:
			return nil
		}
	}
}

enum TomlValue {
	case string(String)
	case int(Int)
	case bool(Bool)
}

enum TomlParser {
	static func parse(_ content: String) -> TomlDictionary {
		var values: [String: TomlValue] = [:]
		var currentTable: String?

		for rawLine in content.split(whereSeparator: \.isNewline) {
			var line = rawLine.trimmingCharacters(in: .whitespacesAndNewlines)
			if line.isEmpty || line.hasPrefix("#") {
				continue
			}
			if line.hasPrefix("[") && line.hasSuffix("]") {
				let table = line.dropFirst().dropLast().trimmingCharacters(in: .whitespaces)
				currentTable = table.isEmpty ? nil : String(table)
				continue
			}
			guard let separatorIndex = line.firstIndex(of: "=") else {
				continue
			}
			let key = line[..<separatorIndex].trimmingCharacters(in: .whitespaces)
			let rawValue = line[line.index(after: separatorIndex)...].trimmingCharacters(in: .whitespaces)
			let value = parseValue(String(rawValue))
			let fullKey = currentTable.map { "\($0).\(key)" } ?? String(key)
			values[fullKey] = value
		}

		return TomlDictionary(values: values)
	}

	private static func parseValue(_ raw: String) -> TomlValue {
		if raw.hasPrefix("\""), let endQuote = raw.dropFirst().firstIndex(of: "\"") {
			let value = raw[raw.index(after: raw.startIndex)..<endQuote]
			return .string(String(value))
		}
		if raw == "true" {
			return .bool(true)
		}
		if raw == "false" {
			return .bool(false)
		}
		if let intValue = Int(raw) {
			return .int(intValue)
		}
		return .string(raw)
	}
}
