import SwiftUI

struct ActivationOrbView: View {
	let isActive: Bool
	#if DEBUG
		@AppStorage("debug.showOrbFrame") private var showsOrbFrame = false
	#endif

	var body: some View {
		LetterMorphOrbView(isActive: isActive)
			.frame(width: UISize.orbDiameter, height: UISize.orbDiameter)
			.overlay {
				#if DEBUG
					if showsOrbFrame {
						Rectangle()
							.stroke(OrbDebug.frameColor, lineWidth: OrbDebug.frameLineWidth)
							.allowsHitTesting(false)
					}
				#endif
			}
	}
}

private struct LetterMorphOrbView: View {
	let isActive: Bool
	@State private var renderer = OrbRenderer()
	@State private var startTime = Date().timeIntervalSinceReferenceDate

	var body: some View {
		TimelineView(.animation(minimumInterval: 1.0 / 30.0)) { timeline in
			Canvas { context, size in
				renderer.render(
					context: &context,
					size: size,
					isActive: isActive,
					startTime: startTime,
					now: timeline.date.timeIntervalSinceReferenceDate
				)
			}
		}
	}
}

private final class OrbRenderer {
	private var transformedPoints: [TransformedPoint] = []
	private var particles: [LetterParticle] = []

	func render(
		context: inout GraphicsContext,
		size: CGSize,
		isActive: Bool,
		startTime: TimeInterval,
		now: TimeInterval
	) {
		let time = now - startTime
		let center = CGPoint(x: size.width / 2, y: size.height / 2)
		let palette = LetterPalette(isActive: isActive)
		let rotation = time * (Double.pi * 2.0 / OrbMotion.rotationPeriod)
		let tilt = OrbMotion.tilt
		let scale = min(size.width, size.height) * OrbMotion.scaleFactor

		context.blendMode = .plusLighter

		let points = letterPoints(at: time, letterPeriod: OrbMotion.rotationPeriod)
		updateTransformedPoints(points: points, time: time, rotation: rotation, tilt: tilt)
		let bounds = updateParticles(center: center, scale: scale, isActive: isActive)
		centerParticlesIfNeeded(bounds: bounds, center: center)

		particles.sort { $0.alpha < $1.alpha }
		drawGlow(context: &context, particles: particles, palette: palette, isActive: isActive)
		drawParticles(context: &context, particles: particles, palette: palette)
	}

	private func updateTransformedPoints(
		points: [CGPoint],
		time: TimeInterval,
		rotation: Double,
		tilt: Double
	) {
		transformedPoints.removeAll(keepingCapacity: true)
		transformedPoints.reserveCapacity(points.count * OrbMotion.layers.count)

		for (index, point) in points.enumerated() {
			let seed = Double(index) * 0.61803398875
			let drift =
				sin(time * OrbMotion.driftSpeed + seed * OrbMotion.driftPhase)
				* OrbMotion.driftAmplitude
			let base = CGPoint(x: point.x + drift, y: point.y - drift)
			let baseDepth = Double(base.x) * OrbMotion.depthScale
			for (layerIndex, layer) in OrbMotion.layers.enumerated() {
				let jitter =
					(rand(seed * OrbMotion.jitterSeedA + Double(layerIndex) * OrbMotion.jitterSeedB)
						- 0.5)
					* OrbMotion.jitterAmplitude
				let transformed = transformPoint(
					base: base,
					depth: baseDepth + layer.depth + jitter,
					rotation: rotation,
					tilt: tilt
				)
				transformedPoints.append(
					TransformedPoint(
						point: transformed.point,
						perspective: transformed.perspective,
						depth: transformed.depth,
						weight: layer.weight,
						face: transformed.face
					)
				)
			}
		}
	}

	private func updateParticles(center: CGPoint, scale: CGFloat, isActive: Bool) -> CGRect {
		particles.removeAll(keepingCapacity: true)
		particles.reserveCapacity(transformedPoints.count)
		var bounds = CGRect.null
		for transformed in transformedPoints {
			let perspective = transformed.perspective
			let position = CGPoint(
				x: center.x + transformed.point.x * scale,
				y: center.y + transformed.point.y * scale
			)
			let face = (0.15 + 0.85 * transformed.face)
			let size =
				(isActive ? 1.9 : 1.6) * transformed.weight * (0.9 + 0.4 * perspective)
				* (0.85 + 0.15 * face)
			let light = clamp(0.7 + transformed.depth * 0.9, min: 0.45, max: 1.0)
			let alpha =
				(isActive ? 0.82 : 0.62) * transformed.weight * (0.55 + 0.45 * perspective) * light
				* face
			let usesAccent = light > 0.84
			let rect = CGRect(
				x: position.x - size / 2,
				y: position.y - size / 2,
				width: size,
				height: size
			)
			bounds = bounds.union(rect)

			particles.append(
				LetterParticle(position: position, size: size, alpha: alpha, usesAccent: usesAccent)
			)
		}
		return bounds
	}

	private func centerParticlesIfNeeded(bounds: CGRect, center: CGPoint) {
		guard !bounds.isNull else {
			return
		}
		let offsetX = center.x - bounds.midX
		let offsetY = center.y - bounds.midY
		guard offsetX != 0 || offsetY != 0 else {
			return
		}
		for index in particles.indices {
			let particle = particles[index]
			particles[index] = LetterParticle(
				position: CGPoint(
					x: particle.position.x + offsetX,
					y: particle.position.y + offsetY),
				size: particle.size,
				alpha: particle.alpha,
				usesAccent: particle.usesAccent
			)
		}
	}

	private func letterPoints(at time: TimeInterval, letterPeriod: TimeInterval) -> [CGPoint] {
		let morphDuration: TimeInterval = letterPeriod * 0.5
		let sequence = LetterCache.sequence
		let index = Int(time / letterPeriod) % sequence.count
		let nextIndex = (index + 1) % sequence.count
		let phase = time.truncatingRemainder(dividingBy: letterPeriod)
		let morphStart = letterPeriod - morphDuration
		let rawProgress = max(0.0, (phase - morphStart) / morphDuration)
		let progress = smoothstep(rawProgress)

		let sourcePoints = LetterCache.centeredPoints[index]
		let targetPoints = LetterCache.centeredPoints[nextIndex]
		if sourcePoints.isEmpty {
			return targetPoints
		}
		if targetPoints.isEmpty {
			return sourcePoints
		}

		return zip(sourcePoints, targetPoints).map { start, end in
			CGPoint(
				x: lerp(start.x, end.x, progress),
				y: lerp(start.y, end.y, progress)
			)
		}
	}

	private func transformPoint(
		base: CGPoint,
		depth: Double,
		rotation: Double,
		tilt: Double
	) -> TransformedPoint {
		let baseX = Double(base.x)
		let baseY = Double(base.y)
		let baseDepth = depth

		let rotatedX = baseX * cos(rotation) + baseDepth * sin(rotation)
		let rotatedZ = -baseX * sin(rotation) + baseDepth * cos(rotation)

		let tiltedY = baseY * cos(tilt) - rotatedZ * sin(tilt)
		let depthZ = baseY * sin(tilt) + rotatedZ * cos(tilt)
		let face = clamp(CGFloat(0.5 + depthZ * 2.2), min: 0.0, max: 1.0)

		let rawPerspective = 1.0 / (1.0 + depthZ * 1.35)
		let perspective = min(1.0, max(0.84, rawPerspective))

		return TransformedPoint(
			point: CGPoint(x: rotatedX * perspective, y: tiltedY * perspective),
			perspective: CGFloat(perspective),
			depth: CGFloat(depthZ),
			weight: 1.0,
			face: face
		)
	}

	private func drawGlow(
		context: inout GraphicsContext,
		particles: [LetterParticle],
		palette: LetterPalette,
		isActive: Bool
	) {
		context.drawLayer { layer in
			layer.addFilter(.blur(radius: 1.2))
			for particle in particles where particle.alpha > 0.7 {
				let size = particle.size * (isActive ? 2.0 : 1.8)
				let rect = CGRect(
					x: particle.position.x - size / 2,
					y: particle.position.y - size / 2,
					width: size,
					height: size
				)
				let color = particle.usesAccent ? palette.accent : palette.highlight
				layer.fill(
					Path(ellipseIn: rect),
					with: .color(color.opacity(0.16 * particle.alpha))
				)
			}
		}
	}

	private func drawParticles(
		context: inout GraphicsContext,
		particles: [LetterParticle],
		palette: LetterPalette
	) {
		for particle in particles {
			let rect = CGRect(
				x: particle.position.x - particle.size / 2,
				y: particle.position.y - particle.size / 2,
				width: particle.size,
				height: particle.size
			)
			let color = particle.usesAccent ? palette.accent : palette.core
			context.fill(
				Path(ellipseIn: rect),
				with: .color(color.opacity(particle.alpha))
			)
		}
	}

	private func smoothstep(_ value: Double) -> Double {
		let progress = min(max(value, 0.0), 1.0)
		return progress * progress * (3.0 - 2.0 * progress)
	}

	private func lerp(_ start: CGFloat, _ end: CGFloat, _ progress: Double) -> CGFloat {
		start + (end - start) * CGFloat(progress)
	}

	private func clamp(_ value: CGFloat, min: CGFloat, max: CGFloat) -> CGFloat {
		Swift.min(Swift.max(value, min), max)
	}

	private func rand(_ value: Double) -> Double {
		fract(sin(value) * 43758.5453)
	}

	private func fract(_ value: Double) -> Double {
		value - floor(value)
	}
}
