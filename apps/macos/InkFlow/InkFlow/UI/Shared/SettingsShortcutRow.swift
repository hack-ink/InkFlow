import SwiftUI

struct SettingsShortcutRow: View {
	let title: String
	@Binding var value: String

	var body: some View {
		HStack {
			Text(title)
				.frame(width: SettingsLayout.shortcutLabelWidth, alignment: .leading)
			TextField("", text: $value)
				.textFieldStyle(.plain)
				.font(.system(size: 13, weight: .medium, design: .monospaced))
				.padding(.vertical, SettingsLayout.shortcutFieldVerticalPadding)
				.padding(.horizontal, SettingsLayout.shortcutFieldHorizontalPadding)
				.background(
					RoundedRectangle(cornerRadius: UICornerRadius.small, style: .continuous)
						.fill(UIColors.shortcutFieldBackground)
				)
		}
	}
}
