import SwiftUI

enum SettingsSection: String, CaseIterable, Identifiable {
	case appearance
	case microphone
	case shortcuts

	var id: String { rawValue }

	var title: String {
		switch self {
		case .appearance:
			return "Appearance"
		case .microphone:
			return "Microphone"
		case .shortcuts:
			return "Shortcuts"
		}
	}
}

struct SettingsSidebarView: View {
	let selectedSection: SettingsSection
	let onSelect: (SettingsSection) -> Void

	var body: some View {
		VStack(alignment: .leading, spacing: SettingsLayout.sidebarSpacing) {
			ForEach(SettingsSection.allCases) { section in
				SettingsSidebarButton(title: section.title, isSelected: selectedSection == section) {
					withAnimation(.easeInOut(duration: UIDuration.selectionChange)) {
						onSelect(section)
					}
				}
			}
		}
		.frame(width: SettingsLayout.sidebarWidth, alignment: .leading)
	}
}

private struct SettingsSidebarButton: View {
	let title: String
	let isSelected: Bool
	let action: () -> Void
	@State private var isHovered = false

	var body: some View {
		Button(action: action) {
			Text(title)
				.font(.system(size: 12, weight: .medium))
				.foregroundStyle(isSelected ? Color.primary : Color.secondary)
				.frame(maxWidth: .infinity, alignment: .leading)
				.padding(.vertical, SettingsLayout.sidebarItemVerticalPadding)
				.padding(.horizontal, SettingsLayout.sidebarItemHorizontalPadding)
				.background(background)
				.clipShape(RoundedRectangle(cornerRadius: UICornerRadius.small, style: .continuous))
		}
		.buttonStyle(.plain)
		.onHover { hovering in
			withAnimation(.easeInOut(duration: UIDuration.hoverFade)) {
				isHovered = hovering
			}
		}
	}

	@ViewBuilder
	private var background: some View {
		if isSelected {
			RoundedRectangle(cornerRadius: UICornerRadius.small, style: .continuous)
				.fill(UIColors.sidebarSelectedBackground)
		} else if isHovered {
			RoundedRectangle(cornerRadius: UICornerRadius.small, style: .continuous)
				.fill(UIColors.sidebarHoverBackground)
		} else {
			Color.clear
		}
	}
}
