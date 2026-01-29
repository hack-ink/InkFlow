import CoreText
import SwiftUI

enum LetterCache {
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

enum LetterSampler {
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
		points.sorted { lhs, rhs in
			let lhsAngle = atan2(lhs.y, lhs.x)
			let rhsAngle = atan2(rhs.y, rhs.x)
			if lhsAngle == rhsAngle {
				let lhsRadius = lhs.x * lhs.x + lhs.y * lhs.y
				let rhsRadius = rhs.x * rhs.x + rhs.y * rhs.y
				return lhsRadius < rhsRadius
			}
			return lhsAngle < rhsAngle
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
				segments.append(
					contentsOf: approximateCurve(
						from: current, control1: control1, control2: control2, to: end))
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
				let progress = CGFloat(index) / CGFloat(steps)
				points.append(segment.interpolate(progress))
			}
		}
		return points
	}

	private static func approximateQuad(from start: CGPoint, control: CGPoint, to end: CGPoint) -> [LineSegment] {
		var segments: [LineSegment] = []
		let steps = 12
		var previous = start
		for index in 1...steps {
			let progress = CGFloat(index) / CGFloat(steps)
			let point = quadBezier(start: start, control: control, end: end, progress: progress)
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
			let progress = CGFloat(index) / CGFloat(steps)
			let point = cubicBezier(
				start: start,
				control1: control1,
				control2: control2,
				end: end,
				progress: progress
			)
			segments.append(LineSegment(start: previous, end: point))
			previous = point
		}
		return segments
	}

	private static func quadBezier(
		start: CGPoint,
		control: CGPoint,
		end: CGPoint,
		progress: CGFloat
	) -> CGPoint {
		let oneMinusT = 1.0 - progress
		let xValue =
			oneMinusT * oneMinusT * start.x
			+ 2.0 * oneMinusT * progress * control.x
			+ progress * progress * end.x
		let yValue =
			oneMinusT * oneMinusT * start.y
			+ 2.0 * oneMinusT * progress * control.y
			+ progress * progress * end.y
		return CGPoint(x: xValue, y: yValue)
	}

	private static func cubicBezier(
		start: CGPoint,
		control1: CGPoint,
		control2: CGPoint,
		end: CGPoint,
		progress: CGFloat
	) -> CGPoint {
		let oneMinusT = 1.0 - progress
		let xValue =
			oneMinusT * oneMinusT * oneMinusT * start.x
			+ 3.0 * oneMinusT * oneMinusT * progress * control1.x
			+ 3.0 * oneMinusT * progress * progress * control2.x
			+ progress * progress * progress * end.x
		let yValue =
			oneMinusT * oneMinusT * oneMinusT * start.y
			+ 3.0 * oneMinusT * oneMinusT * progress * control1.y
			+ 3.0 * oneMinusT * progress * progress * control2.y
			+ progress * progress * progress * end.y
		return CGPoint(x: xValue, y: yValue)
	}

	private static func fillPoints(path: CGPath, bounds: CGRect, step: CGFloat) -> [CGPoint] {
		var points: [CGPoint] = []
		var currentY = bounds.minY
		while currentY <= bounds.maxY {
			var currentX = bounds.minX
			while currentX <= bounds.maxX {
				let point = CGPoint(x: currentX, y: currentY)
				if path.contains(point, using: .winding, transform: .identity) {
					points.append(point)
				}
				currentX += step
			}
			currentY += step
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

struct LineSegment {
	let start: CGPoint
	let end: CGPoint

	var length: CGFloat {
		let deltaX = end.x - start.x
		let deltaY = end.y - start.y
		return sqrt(deltaX * deltaX + deltaY * deltaY)
	}

	func interpolate(_ progress: CGFloat) -> CGPoint {
		CGPoint(
			x: start.x + (end.x - start.x) * progress,
			y: start.y + (end.y - start.y) * progress
		)
	}
}

struct LetterPalette {
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

struct LetterParticle {
	let position: CGPoint
	let size: CGFloat
	let alpha: CGFloat
	let usesAccent: Bool
}

struct TransformedPoint {
	let point: CGPoint
	let perspective: CGFloat
	let depth: CGFloat
	let weight: CGFloat
	let face: CGFloat
}

enum OrbMotion {
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

enum OrbDebug {
	static let frameLineWidth: CGFloat = 1
	static let frameColor = Color.red.opacity(0.6)
}
