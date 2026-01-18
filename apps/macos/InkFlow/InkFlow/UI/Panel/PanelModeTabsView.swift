import SwiftUI

enum PanelMode: String, CaseIterable, Identifiable {
	case history
	case clips
	case notes

	var id: String { rawValue }

	var title: String {
		switch self {
		case .history:
			return "History"
		case .clips:
			return "Clips"
		case .notes:
			return "Notes"
		}
	}
}

struct PanelModeTabsView: View {
	let selectedMode: PanelMode
	let onSelect: (PanelMode) -> Void

	var body: some View {
		HStack(spacing: PanelExpandedLayout.modeBarSpacing) {
			ForEach(PanelMode.allCases) { mode in
				PanelModeTab(title: mode.title, isSelected: selectedMode == mode) {
					withAnimation(.easeInOut(duration: UIDuration.selectionChange)) {
						onSelect(mode)
					}
				}
			}
			Spacer(minLength: 0)
		}
		.frame(maxWidth: .infinity, alignment: .leading)
	}
}

private struct PanelModeTab: View {
	let title: String
	let isSelected: Bool
	let action: () -> Void

	var body: some View {
		Button(action: action) {
			VStack(spacing: PanelModeTabLayout.spacing) {
				Text(title)
					.font(.system(size: 12, weight: .semibold))
					.foregroundStyle(isSelected ? Color.primary : Color.secondary)
					.lineLimit(1)
					.truncationMode(.tail)
				Rectangle()
					.frame(height: UISize.modeUnderlineHeight)
					.foregroundStyle(isSelected ? UIColors.modeTabIndicatorSelected : UIColors.modeTabIndicatorUnselected)
			}
		}
		.buttonStyle(.plain)
		.animation(.easeInOut(duration: UIDuration.selectionChange), value: isSelected)
	}
}
