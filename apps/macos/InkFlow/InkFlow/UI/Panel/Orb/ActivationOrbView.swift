import CoreText
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
		transformedPoints.removeAll(keepingCapacity: true)
		transformedPoints.reserveCapacity(points.count * OrbMotion.layers.count)

		for (index, point) in points.enumerated() {
			let seed = Double(index) * 0.61803398875
			let drift = sin(time * OrbMotion.driftSpeed + seed * OrbMotion.driftPhase)
				* OrbMotion.driftAmplitude
			let base = CGPoint(x: point.x + drift, y: point.y - drift)
			let baseDepth = Double(base.x) * OrbMotion.depthScale
			for (layerIndex, layer) in OrbMotion.layers.enumerated() {
				let jitter = (rand(seed * OrbMotion.jitterSeedA + Double(layerIndex) * OrbMotion.jitterSeedB) - 0.5)
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
			let size = (isActive ? 1.9 : 1.6) * transformed.weight * (0.9 + 0.4 * perspective) * (0.85 + 0.15 * face)
			let light = clamp(0.7 + transformed.depth * 0.9, min: 0.45, max: 1.0)
			let alpha = (isActive ? 0.82 : 0.62) * transformed.weight * (0.55 + 0.45 * perspective) * light * face
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
		if !bounds.isNull {
			let offsetX = center.x - bounds.midX
			let offsetY = center.y - bounds.midY
			if offsetX != 0 || offsetY != 0 {
				for index in particles.indices {
					let particle = particles[index]
					particles[index] = LetterParticle(
						position: CGPoint(x: particle.position.x + offsetX, y: particle.position.y + offsetY),
						size: particle.size,
						alpha: particle.alpha,
						usesAccent: particle.usesAccent
					)
				}
			}
		}

		particles.sort { $0.alpha < $1.alpha }
		drawGlow(context: &context, particles: particles, palette: palette, isActive: isActive)
		drawParticles(context: &context, particles: particles, palette: palette)
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

		let from = LetterCache.centeredPoints[index]
		let to = LetterCache.centeredPoints[nextIndex]
		if from.isEmpty {
			return to
		}
		if to.isEmpty {
			return from
		}

		return zip(from, to).map { start, end in
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
		let x = Double(base.x)
		let y = Double(base.y)
		let z = depth

		let rotatedX = x * cos(rotation) + z * sin(rotation)
		let rotatedZ = -x * sin(rotation) + z * cos(rotation)

		let tiltedY = y * cos(tilt) - rotatedZ * sin(tilt)
		let depthZ = y * sin(tilt) + rotatedZ * cos(tilt)
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
		let t = min(max(value, 0.0), 1.0)
		return t * t * (3.0 - 2.0 * t)
	}

	private func lerp(_ start: CGFloat, _ end: CGFloat, _ t: Double) -> CGFloat {
		start + (end - start) * CGFloat(t)
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

private enum LetterCache {
	static let sequence: [Character] = ["I", "N", "K", "F", "L", "O", "W"]
	static let pointCount = 220
	static let samples: [(points: [CGPoint], centroid: CGPoint)] = sequence.map {
		LetterSampler.sample(for: $0, count: pointCount)
	}
	static let points: [[CGPoint]] = samples.map { $0.points }
	// Center each glyph on its centroid to reduce per-letter drift.
	static let centeredPoints: [[CGPoint]] = zip(points, centroids).map { glyphPoints, centroid in
		glyphPoints.map { point in
			CGPoint(x: point.x - centroid.x, y: point.y - centroid.y)
		}
	}
	static let centroids: [CGPoint] = samples.map { $0.centroid }
}

private enum LetterSampler {
	private static let font: CTFont = {
		let candidates = ["SFMono-Bold", "Menlo-Bold", "Courier-Bold", ".SFNSDisplay"]
		for name in candidates {
			let font = CTFontCreateWithName(name as CFString, 1.0, nil)
			if name == ".SFNSDisplay" {
				return font
			}
			let postScript = CTFontCopyPostScriptName(font) as String
			if postScript == name {
				return font
			}
		}
		return CTFontCreateWithName(".SFNSDisplay" as CFString, 1.0, nil)
	}()

	static func sample(for character: Character, count: Int) -> (points: [CGPoint], centroid: CGPoint) {
		guard let path = path(for: character) else {
			return ([], .zero)
		}
		let normalized = normalize(path: path)
		let bounds = normalized.boundingBoxOfPath
		let isK = character == "K"
		let outlineStep: CGFloat = isK ? 0.02 : 0.025
		let fillStep: CGFloat = isK ? 0.085 : 0.05
		let outline = outlinePoints(path: normalized, step: outlineStep)
		let fill = fillPoints(path: normalized, bounds: bounds, step: fillStep)
		let centroid = centroid(for: fill.isEmpty ? outline : fill)
		let pool = sortPoints(points: outline + fill)
		return (resample(points: pool, count: count), centroid)
	}

	private static func path(for character: Character) -> CGPath? {
		let scalars = Array(String(character).utf16)
		guard let uni = scalars.first else {
			return nil
		}
		var glyph = CGGlyph()
		var char = uni
		guard CTFontGetGlyphsForCharacters(font, &char, &glyph, 1) else {
			return nil
		}
		return CTFontCreatePathForGlyph(font, glyph, nil)
	}

	private static func normalize(path: CGPath) -> CGPath {
		let bounds = path.boundingBoxOfPath
		let scale = 1.0 / max(bounds.width, bounds.height)
		var transform = CGAffineTransform.identity
		transform = transform.translatedBy(x: -bounds.midX, y: -bounds.midY)
		transform = transform.scaledBy(x: scale, y: -scale)
		return path.copy(using: &transform) ?? path
	}

	private static func centroid(for points: [CGPoint]) -> CGPoint {
		guard !points.isEmpty else {
			return .zero
		}
		var sumX: CGFloat = 0
		var sumY: CGFloat = 0
		for point in points {
			sumX += point.x
			sumY += point.y
		}
		let count = CGFloat(points.count)
		return CGPoint(x: sumX / count, y: sumY / count)
	}

	private static func sortPoints(points: [CGPoint]) -> [CGPoint] {
		points.sorted { a, b in
			let angleA = atan2(a.y, a.x)
			let angleB = atan2(b.y, b.x)
			if angleA == angleB {
				let radiusA = a.x * a.x + a.y * a.y
				let radiusB = b.x * b.x + b.y * b.y
				return radiusA < radiusB
			}
			return angleA < angleB
		}
	}

	private static func outlinePoints(path: CGPath, step: CGFloat) -> [CGPoint] {
		var segments: [LineSegment] = []
		var current = CGPoint.zero
		var start = CGPoint.zero

		path.applyWithBlock { elementPointer in
			let element = elementPointer.pointee
			switch element.type {
			case .moveToPoint:
				current = element.points[0]
				start = current
			case .addLineToPoint:
				let next = element.points[0]
				segments.append(LineSegment(start: current, end: next))
				current = next
			case .addQuadCurveToPoint:
				let control = element.points[0]
				let end = element.points[1]
				segments.append(contentsOf: approximateQuad(from: current, control: control, to: end))
				current = end
			case .addCurveToPoint:
				let control1 = element.points[0]
				let control2 = element.points[1]
				let end = element.points[2]
				segments.append(contentsOf: approximateCurve(from: current, control1: control1, control2: control2, to: end))
				current = end
			case .closeSubpath:
				segments.append(LineSegment(start: current, end: start))
				current = start
			@unknown default:
				break
			}
		}

		var points: [CGPoint] = []
		for segment in segments {
			let length = segment.length
			let steps = max(2, Int(length / step))
			for index in 0...steps {
				let t = CGFloat(index) / CGFloat(steps)
				points.append(segment.interpolate(t))
			}
		}
		return points
	}

	private static func approximateQuad(from start: CGPoint, control: CGPoint, to end: CGPoint) -> [LineSegment] {
		var segments: [LineSegment] = []
		let steps = 12
		var previous = start
		for index in 1...steps {
			let t = CGFloat(index) / CGFloat(steps)
			let point = quadBezier(start: start, control: control, end: end, t: t)
			segments.append(LineSegment(start: previous, end: point))
			previous = point
		}
		return segments
	}

	private static func approximateCurve(
		from start: CGPoint,
		control1: CGPoint,
		control2: CGPoint,
		to end: CGPoint
	) -> [LineSegment] {
		var segments: [LineSegment] = []
		let steps = 16
		var previous = start
		for index in 1...steps {
			let t = CGFloat(index) / CGFloat(steps)
			let point = cubicBezier(start: start, control1: control1, control2: control2, end: end, t: t)
			segments.append(LineSegment(start: previous, end: point))
			previous = point
		}
		return segments
	}

	private static func quadBezier(start: CGPoint, control: CGPoint, end: CGPoint, t: CGFloat) -> CGPoint {
		let mt = 1.0 - t
		let x = mt * mt * start.x + 2.0 * mt * t * control.x + t * t * end.x
		let y = mt * mt * start.y + 2.0 * mt * t * control.y + t * t * end.y
		return CGPoint(x: x, y: y)
	}

	private static func cubicBezier(
		start: CGPoint,
		control1: CGPoint,
		control2: CGPoint,
		end: CGPoint,
		t: CGFloat
	) -> CGPoint {
		let mt = 1.0 - t
		let x = mt * mt * mt * start.x
			+ 3.0 * mt * mt * t * control1.x
			+ 3.0 * mt * t * t * control2.x
			+ t * t * t * end.x
		let y = mt * mt * mt * start.y
			+ 3.0 * mt * mt * t * control1.y
			+ 3.0 * mt * t * t * control2.y
			+ t * t * t * end.y
		return CGPoint(x: x, y: y)
	}

	private static func fillPoints(path: CGPath, bounds: CGRect, step: CGFloat) -> [CGPoint] {
		var points: [CGPoint] = []
		var y = bounds.minY
		while y <= bounds.maxY {
			var x = bounds.minX
			while x <= bounds.maxX {
				let point = CGPoint(x: x, y: y)
				if path.contains(point, using: .winding, transform: .identity) {
					points.append(point)
				}
				x += step
			}
			y += step
		}
		return points
	}

	private static func resample(points: [CGPoint], count: Int) -> [CGPoint] {
		guard !points.isEmpty, count > 0 else {
			return []
		}
		if points.count >= count {
			let step = Double(points.count - 1) / Double(count - 1)
			return (0..<count).map { index in
				let sampleIndex = Int(Double(index) * step)
				return points[min(sampleIndex, points.count - 1)]
			}
		}
		return (0..<count).map { index in
			points[index % points.count]
		}
	}
}

private struct LineSegment {
	let start: CGPoint
	let end: CGPoint

	var length: CGFloat {
		let dx = end.x - start.x
		let dy = end.y - start.y
		return sqrt(dx * dx + dy * dy)
	}

	func interpolate(_ t: CGFloat) -> CGPoint {
		CGPoint(
			x: start.x + (end.x - start.x) * t,
			y: start.y + (end.y - start.y) * t
		)
	}
}

private struct LetterPalette {
	let core: Color
	let accent: Color
	let highlight: Color

	init(isActive: Bool) {
		if isActive {
			core = UIOrbPalette.activeCore
			accent = UIOrbPalette.activeAccent
			highlight = UIOrbPalette.activeHighlight
		} else {
			core = UIOrbPalette.inactiveCore
			accent = UIOrbPalette.inactiveAccent
			highlight = UIOrbPalette.inactiveHighlight
		}
	}
}

private struct LetterParticle {
	let position: CGPoint
	let size: CGFloat
	let alpha: CGFloat
	let usesAccent: Bool
}

private struct TransformedPoint {
	let point: CGPoint
	let perspective: CGFloat
	let depth: CGFloat
	let weight: CGFloat
	let face: CGFloat
}

private enum OrbMotion {
	static let rotationPeriod: TimeInterval = 6.0
	static let tilt: Double = 0.14
	static let scaleFactor: CGFloat = 0.88
	static let driftSpeed: Double = 0.8
	static let driftPhase: Double = 2.3
	static let driftAmplitude: Double = 0.004
	static let depthScale: Double = 0.32
	static let jitterSeedA: Double = 6.7
	static let jitterSeedB: Double = 1.9
	static let jitterAmplitude: Double = 0.04
	static let layers: [(depth: Double, weight: CGFloat)] = [
		(-0.12, 0.6),
		(-0.06, 0.8),
		(0.0, 1.0),
		(0.06, 0.8),
		(0.12, 0.6)
	]
}

private enum OrbDebug {
	static let frameLineWidth: CGFloat = 1
	static let frameColor = Color.red.opacity(0.6)
}
