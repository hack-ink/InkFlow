import SwiftUI

struct ParticleFieldView: View {
	let isActive: Bool

	var body: some View {
		TimelineView(.animation(minimumInterval: 1.0 / 24.0)) { timeline in
			Canvas { context, size in
				let now = timeline.date.timeIntervalSinceReferenceDate
				let count = isActive ? 18 : 12
				let drift = isActive ? 1.3 : 0.8
				let radius = isActive ? 6.0 : 5.0
				let alpha = isActive ? 0.12 : 0.08
				let color = Color(red: 0.6, green: 0.85, blue: 0.95).opacity(alpha)

				for index in 0..<count {
					let seed = Double(index) * 0.61803398875
					let baseX = fract(sin(seed * 32.1) * 43758.5453)
					let baseY = fract(sin(seed * 91.7) * 24634.6345)
					let wobbleX = sin(now * 0.2 + seed * 8.0) * drift
					let wobbleY = cos(now * 0.18 + seed * 10.0) * drift
					let x = CGFloat(baseX) * size.width + CGFloat(wobbleX)
					let y = CGFloat(baseY) * size.height + CGFloat(wobbleY)
					let rect = CGRect(x: x - radius / 2, y: y - radius / 2, width: radius, height: radius)
					context.fill(Path(ellipseIn: rect), with: .color(color))
				}
			}
		}
	}

	private func fract(_ value: Double) -> Double {
		value - floor(value)
	}
}
