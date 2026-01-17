import SwiftUI

enum ThemePreference: String, CaseIterable, Identifiable {
	case system
	case light
	case dark

	var id: String { rawValue }

	var title: String {
		switch self {
		case .system:
			return "System"
		case .light:
			return "Light"
		case .dark:
			return "Dark"
		}
	}

	var preferredColorScheme: ColorScheme? {
		switch self {
		case .system:
			return nil
		case .light:
			return .light
		case .dark:
			return .dark
		}
	}
}

enum AccentOption: String, CaseIterable, Identifiable {
	case coral
	case sky
	case mint
	case amber
	case violet
	case graphite

	var id: String { rawValue }

	var title: String {
		switch self {
		case .coral:
			return "Coral"
		case .sky:
			return "Sky"
		case .mint:
			return "Mint"
		case .amber:
			return "Amber"
		case .violet:
			return "Violet"
		case .graphite:
			return "Graphite"
		}
	}

	var color: Color {
		switch self {
		case .coral:
			return Color(red: 1.0, green: 0.47, blue: 0.48)
		case .sky:
			return Color(red: 0.33, green: 0.71, blue: 0.98)
		case .mint:
			return Color(red: 0.25, green: 0.82, blue: 0.67)
		case .amber:
			return Color(red: 0.98, green: 0.69, blue: 0.2)
		case .violet:
			return Color(red: 0.62, green: 0.46, blue: 0.98)
		case .graphite:
			return Color(red: 0.38, green: 0.43, blue: 0.5)
		}
	}
}

enum GlassIntensity: String, CaseIterable, Identifiable {
	case subtle
	case standard
	case vivid

	var id: String { rawValue }

	var title: String {
		switch self {
		case .subtle:
			return "Subtle"
		case .standard:
			return "Standard"
		case .vivid:
			return "Vivid"
		}
	}

	var surfaceOpacity: Double {
		switch self {
		case .subtle:
			return 0.18
		case .standard:
			return 0.26
		case .vivid:
			return 0.34
		}
	}

	var fieldOpacity: Double {
		switch self {
		case .subtle:
			return 0.08
		case .standard:
			return 0.12
		case .vivid:
			return 0.16
		}
	}

	var windowFillOpacity: Double {
		switch self {
		case .subtle:
			return 0.62
		case .standard:
			return 0.72
		case .vivid:
			return 0.8
		}
	}
}

struct AppearanceStyle {
	let theme: ThemePreference
	let accent: AccentOption
	let glassIntensity: GlassIntensity
	let isTranslucent: Bool

	var preferredColorScheme: ColorScheme? {
		theme.preferredColorScheme
	}

	var accentColor: Color {
		accent.color
	}

	var surfaceTint: Color {
		Color.white.opacity(glassIntensity.surfaceOpacity)
	}

	var fieldTint: Color {
		Color.white.opacity(glassIntensity.fieldOpacity)
	}

	var windowFill: Color {
		Color(nsColor: .windowBackgroundColor).opacity(glassIntensity.windowFillOpacity)
	}

	static func theme(from raw: String) -> ThemePreference {
		ThemePreference(rawValue: raw) ?? .system
	}

	static func accent(from raw: String) -> AccentOption {
		AccentOption(rawValue: raw) ?? .sky
	}

	static func glassIntensity(from raw: String) -> GlassIntensity {
		GlassIntensity(rawValue: raw) ?? .standard
	}
}
