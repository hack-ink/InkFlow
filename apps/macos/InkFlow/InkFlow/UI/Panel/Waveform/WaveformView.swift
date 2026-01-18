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
		let safeLevels = levels.isEmpty ? WaveformLayout.defaultLevels : levels
		GeometryReader { proxy in
			let barCount = safeLevels.count
			let spacing = WaveformLayout.barSpacing
			let totalSpacing = spacing * CGFloat(max(barCount - 1, 0))
			let barWidth = max((proxy.size.width - totalSpacing) / CGFloat(barCount), WaveformLayout.minimumBarWidth)
			let topColor = isActive ? UIColors.waveformActiveTop : UIColors.waveformInactiveTop
			let bottomColor = isActive ? UIColors.waveformActiveBottom : UIColors.waveformInactiveBottom
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
		.animation(.easeOut(duration: UIDuration.waveformLevel), value: levels)
	}

	private func normalized(_ value: CGFloat) -> CGFloat {
		min(max(value, WaveformLayout.minimumLevel), 1.0)
	}
}

private enum WaveformLayout {
	static let defaultBarCount: Int = 28
	static let defaultLevel: CGFloat = 0.04
	static let minimumLevel: CGFloat = 0.03
	static let barSpacing: CGFloat = 3
	static let minimumBarWidth: CGFloat = 2
	static let defaultLevels: [CGFloat] = Array(repeating: defaultLevel, count: defaultBarCount)
}
