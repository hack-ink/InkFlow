import SwiftUI

struct AppearanceAccentGroup: View {
	@Binding var accent: AccentOption

	var body: some View {
		SettingsGroupView(title: "Accent Color") {
			LazyVGrid(
				columns: Array(
					repeating: GridItem(
						.fixed(SettingsLayout.accentGridItemSize),
						spacing: SettingsLayout.accentGridSpacing
					),
					count: SettingsLayout.accentGridColumns
				),
				spacing: SettingsLayout.accentGridSpacing
			) {
				ForEach(AccentOption.allCases) { option in
					Button {
						withAnimation(.easeInOut(duration: UIDuration.selectionChange)) {
							accent = option
						}
					} label: {
						Circle()
							.fill(option.color)
							.frame(
								width: UISize.accentSwatch,
								height: UISize.accentSwatch
							)
							.overlay(selectionRing(for: option))
					}
					.buttonStyle(.plain)
					.accessibilityLabel(option.title)
				}
			}
		}
	}

	@ViewBuilder
	private func selectionRing(for option: AccentOption) -> some View {
		if accent == option {
			Circle()
				.strokeBorder(UIColors.selectionRing, lineWidth: SettingsLayout.selectionRingLineWidth)
		}
	}
}
