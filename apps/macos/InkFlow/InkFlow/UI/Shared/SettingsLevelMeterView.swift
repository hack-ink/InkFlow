import SwiftUI

struct SettingsLevelMeterView: View {
	let level: CGFloat
	let isActive: Bool

	var body: some View {
		GeometryReader { proxy in
			let width = proxy.size.width
			let height = proxy.size.height
			let filled = max(min(level, 1), 0) * width

			ZStack(alignment: .leading) {
				RoundedRectangle(cornerRadius: height / 2, style: .continuous)
					.fill(UIColors.levelMeterTrack)
				RoundedRectangle(cornerRadius: height / 2, style: .continuous)
					.fill(isActive ? UIColors.levelMeterActiveFill : UIColors.levelMeterInactiveFill)
					.frame(width: filled)
			}
		}
		.frame(height: UISize.levelMeterHeight)
		.animation(.easeOut(duration: UIDuration.meterLevel), value: level)
		.animation(.easeInOut(duration: UIDuration.standard), value: isActive)
	}
}
