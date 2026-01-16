import SwiftUI

struct ActivationOrbView: View {
	let isActive: Bool

	var body: some View {
		CollisionOrbView(isActive: isActive)
			.frame(width: 28, height: 28)
	}
}

private struct CollisionOrbView: View {
	let isActive: Bool

	var body: some View {
		TimelineView(.animation(minimumInterval: 1.0 / 30.0)) { timeline in
			Canvas { context, size in
				let now = timeline.date.timeIntervalSinceReferenceDate
				let center = CGPoint(x: size.width / 2, y: size.height / 2)
				let maxRadius = min(size.width, size.height) * 0.42
				let particles = particleStates(now: now, center: center, maxRadius: maxRadius)

				context.blendMode = .plusLighter
				drawParticles(context: &context, particles: particles)
				drawSparks(context: &context, particles: particles)
			}
		}
	}

	private func particleStates(
		now: TimeInterval,
		center: CGPoint,
		maxRadius: CGFloat
	) -> [OrbParticle] {
		let count = isActive ? 22 : 16
		let baseSpeed = isActive ? 0.9 : 0.55
		let jitter = isActive ? 2.2 : 1.4
		var result: [OrbParticle] = []
		result.reserveCapacity(count)
		for index in 0..<count {
			let seed = Double(index) * 0.61803398875
			let speed = baseSpeed + fract(sin(seed * 12.3) * 13.7) * 0.6
			let angle = now * speed + seed * Double.pi * 2
			let radial = maxRadius * CGFloat(0.35 + 0.6 * fract(sin(seed * 78.2) * 43758.5453))
			let wobble = sin(now * 1.6 + seed * 9.0) * jitter
			let x = center.x + cos(angle) * radial + CGFloat(wobble)
			let y = center.y + sin(angle * 1.1) * radial + CGFloat(wobble)
			let size = isActive ? 1.6 : 1.2
			let intensity = 0.6 + 0.4 * CGFloat(fract(sin(seed * 31.7) * 24634.6345))
			result.append(OrbParticle(position: CGPoint(x: x, y: y), size: size, intensity: intensity))
		}
		return result
	}

	private func drawParticles(context: inout GraphicsContext, particles: [OrbParticle]) {
		let baseColor = isActive
			? Color(red: 0.35, green: 0.95, blue: 1.0)
			: Color(red: 0.65, green: 0.9, blue: 1.0)
		for particle in particles {
			let alpha = (isActive ? 0.85 : 0.6) * particle.intensity
			let rect = CGRect(
				x: particle.position.x - particle.size / 2,
				y: particle.position.y - particle.size / 2,
				width: particle.size,
				height: particle.size
			)
			context.fill(Path(ellipseIn: rect), with: .color(baseColor.opacity(alpha)))
		}
	}

	private func drawSparks(context: inout GraphicsContext, particles: [OrbParticle]) {
		let threshold: CGFloat = isActive ? 3.8 : 3.0
		let sparkColor = Color(red: 0.35, green: 0.8, blue: 1.0)
		for i in 0..<particles.count {
			for j in (i + 1)..<particles.count {
				let a = particles[i].position
				let b = particles[j].position
				let dx = a.x - b.x
				let dy = a.y - b.y
				let distance = sqrt(dx * dx + dy * dy)
				if distance >= threshold {
					continue
				}
				let intensity = max(0.0, (threshold - distance) / threshold)
				let mid = CGPoint(x: (a.x + b.x) / 2, y: (a.y + b.y) / 2)
				let sparkCount = isActive ? 4 : 3
				let baseAngle = Double(i * 17 + j * 11) * 0.3
				for index in 0..<sparkCount {
					let angle = baseAngle + Double(index) * Double.pi * 2 / Double(sparkCount)
					let length = (isActive ? 4.2 : 3.0) * Double(intensity)
					let end = CGPoint(
						x: mid.x + CGFloat(cos(angle) * length),
						y: mid.y + CGFloat(sin(angle) * length)
					)
					var path = Path()
					path.move(to: mid)
					path.addLine(to: end)
					context.stroke(
						path,
						with: .color(sparkColor.opacity(0.9 * intensity)),
						lineWidth: 0.8
					)
				}
				let glowRect = CGRect(x: mid.x - 1.5, y: mid.y - 1.5, width: 3, height: 3)
				context.fill(Path(ellipseIn: glowRect), with: .color(sparkColor.opacity(0.6 * intensity)))
			}
		}
	}

	private func fract(_ value: Double) -> Double {
		value - floor(value)
	}
}

private struct OrbParticle {
	let position: CGPoint
	let size: CGFloat
	let intensity: CGFloat
}
