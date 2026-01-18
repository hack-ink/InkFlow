import SwiftUI

struct SettingsGroupView<Content: View>: View {
	let title: String
	let content: Content

	init(title: String, @ViewBuilder content: () -> Content) {
		self.title = title
		self.content = content()
	}

	var body: some View {
		VStack(alignment: .leading, spacing: SettingsLayout.groupSpacing) {
			Text(title)
				.font(.caption.weight(.semibold))
				.foregroundStyle(.secondary)
			content
		}
		.frame(maxWidth: .infinity, alignment: .leading)
	}
}
