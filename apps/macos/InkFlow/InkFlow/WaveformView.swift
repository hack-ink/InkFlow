import SwiftUI

struct WaveformView: View {
	enum Style {
		case bars
	}

	let levels: [CGFloat]
	let style: Style
	let isActive: Bool

	init(levels: [CGFloat], style: Style = .bars, isActive: Bool) {
		self.levels = levels
		self.style = style
		self.isActive = isActive
	}

	var body: some View {
		switch style {
		case .bars:
			WaveformBars(levels: levels, isActive: isActive)
		}
	}
}

private struct WaveformBars: View {
	let levels: [CGFloat]
	let isActive: Bool

	var body: some View {
		let safeLevels = levels.isEmpty ? Array(repeating: 0.04, count: 28) : levels
		GeometryReader { proxy in
			let barCount = safeLevels.count
			let spacing: CGFloat = 3
			let totalSpacing = spacing * CGFloat(max(barCount - 1, 0))
			let barWidth = max((proxy.size.width - totalSpacing) / CGFloat(barCount), 2)
			let topColor = Color.white.opacity(isActive ? 0.9 : 0.35)
			let bottomColor = Color.white.opacity(isActive ? 0.45 : 0.18)
			let gradient = LinearGradient(colors: [topColor, bottomColor], startPoint: .top, endPoint: .bottom)

			HStack(alignment: .center, spacing: spacing) {
				ForEach(safeLevels.indices, id: \.self) { index in
					let level = normalized(safeLevels[index])
					Capsule(style: .continuous)
						.fill(gradient)
						.frame(width: barWidth, height: proxy.size.height * level)
						.frame(maxHeight: .infinity, alignment: .center)
				}
			}
			.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)
		}
		.animation(.easeOut(duration: 0.08), value: levels)
	}

	private func normalized(_ value: CGFloat) -> CGFloat {
		min(max(value, 0.03), 1.0)
	}
}
