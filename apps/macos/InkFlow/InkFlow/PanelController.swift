import AppKit
import Combine
import Foundation
import QuartzCore

final class PanelController: ObservableObject {
	@Published private(set) var isExpanded = false
	@Published private(set) var isSettingsPresented = false
	weak var panel: NSPanel?

	private let collapsedHeight: CGFloat = 60
	private let expandedHeight: CGFloat = 360
	private let panelWidth: CGFloat = 720
	private let expandedCornerRadius: CGFloat = 24
	private let animationDuration: TimeInterval = 0.32
	private let timingFunction = CAMediaTimingFunction(controlPoints: 0.22, 0.61, 0.36, 1.0)
	private let maskOversample: CGFloat = 4.0
	private let isExpansionLocked = false

	var expandedPanelHeight: CGFloat {
		expandedHeight
	}

	var collapsedPanelHeight: CGFloat {
		collapsedHeight
	}

	func toggleSettings() {
		if !isExpanded {
			setExpanded(true, animated: true)
		}
		isSettingsPresented.toggle()
	}

	func toggleExpanded() {
		setExpanded(!isExpanded, animated: true)
	}

	func closeSettings() {
		isSettingsPresented = false
	}

	func setExpanded(_ expanded: Bool, animated: Bool) {
		if isExpansionLocked, expanded {
			return
		}
		guard expanded != isExpanded else {
			return
		}
		isExpanded = expanded
		if !expanded {
			isSettingsPresented = false
		}
		let targetHeight = expanded ? expandedHeight : collapsedHeight
		updatePanelHeight(targetHeight, animated: animated)
	}

	func handleExitCommand() {
		if isSettingsPresented {
			closeSettings()
		} else if isExpanded {
			setExpanded(false, animated: true)
		} else {
			hidePanel()
		}
	}

	func syncPanelSize(animated: Bool) {
		let targetHeight = isExpanded ? expandedHeight : collapsedHeight
		updatePanelHeight(targetHeight, animated: animated)
	}

	func showPanel() {
		guard let panel else {
			return
		}
		NSApp.activate(ignoringOtherApps: true)
		panel.makeKeyAndOrderFront(nil)
	}

	func hidePanel() {
		panel?.orderOut(nil)
	}

	private func updatePanelHeight(_ height: CGFloat, animated: Bool) {
		guard let panel else {
			return
		}
		let currentFrame = panel.frame
		let topEdge = currentFrame.maxY
		var frame = currentFrame
		frame.size.height = height
		frame.size.width = panelWidth
		frame.origin.y = topEdge - height
		let targetCornerRadius = isExpanded ? expandedCornerRadius : max(0, height / 2)
		if animated {
			NSAnimationContext.runAnimationGroup { context in
				context.duration = animationDuration
				context.timingFunction = timingFunction
				panel.animator().setFrame(frame, display: true)
			} completionHandler: { [weak self] in
				guard let self else {
					return
				}
				let alignedFrame = panel.backingAlignedRect(panel.frame, options: .alignAllEdgesNearest)
				panel.setFrame(alignedFrame, display: true)
				self.updatePanelCornerRadius(targetCornerRadius, animated: false)
			}
		} else {
			frame = panel.backingAlignedRect(frame, options: .alignAllEdgesNearest)
			panel.setFrame(frame, display: true)
		}
		updatePanelCornerRadius(targetCornerRadius, animated: animated)
	}

	private func updatePanelCornerRadius(_ cornerRadius: CGFloat, animated: Bool) {
		guard let panel else {
			return
		}
		let scale = panel.screen?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2.0

		let applyCornerRadius: (NSView, Bool, Bool) -> Void = { view, applyMask, useBoundsRadius in
			view.wantsLayer = true
			view.layoutSubtreeIfNeeded()
			guard let layer = view.layer else {
				return
			}
			layer.contentsScale = scale
			layer.allowsEdgeAntialiasing = true
			let boundsRadius = min(layer.bounds.width, layer.bounds.height) / 2
			let resolvedRadius: CGFloat
			if animated {
				resolvedRadius = cornerRadius
			} else if useBoundsRadius {
				resolvedRadius = boundsRadius
			} else {
				resolvedRadius = cornerRadius
			}
			let alignedRadius = (resolvedRadius * scale).rounded(.toNearestOrAwayFromZero) / scale
			if animated {
				let animation = CABasicAnimation(keyPath: "cornerRadius")
				animation.fromValue = layer.presentation()?.cornerRadius ?? layer.cornerRadius
				animation.toValue = alignedRadius
				animation.duration = self.animationDuration
				animation.timingFunction = self.timingFunction
				layer.add(animation, forKey: "cornerRadius")
			}
			layer.cornerRadius = alignedRadius
			layer.cornerCurve = .circular
			layer.masksToBounds = true
			if applyMask {
				if animated {
					layer.mask = nil
				} else {
					self.applyMaskImage(to: layer, cornerRadius: alignedRadius, scale: scale)
				}
			} else {
				layer.mask = nil
			}
		}

		if let contentView = panel.contentView {
			applyCornerRadius(contentView, false, !isExpanded)
		}

		if let contentView = panel.contentViewController?.view {
			applyCornerRadius(contentView, false, !isExpanded)
		}

		if let frameView = panel.contentView?.superview {
			applyCornerRadius(frameView, true, !isExpanded)
		}

		panel.invalidateShadow()
	}

	private func applyMaskImage(to layer: CALayer, cornerRadius: CGFloat, scale: CGFloat) {
		let size = layer.bounds.size
		if size.width <= 0 || size.height <= 0 {
			layer.mask = nil
			return
		}
		let maskScale = scale * maskOversample
		let maskLayer = layer.mask ?? CALayer()
		maskLayer.frame = layer.bounds
		maskLayer.contentsGravity = .resize
		maskLayer.magnificationFilter = .linear
		maskLayer.minificationFilter = .trilinear
		maskLayer.contentsScale = maskScale
		maskLayer.contents = makeMaskImage(size: size, cornerRadius: cornerRadius, scale: maskScale)
		layer.mask = maskLayer
	}

	private func makeMaskImage(size: CGSize, cornerRadius: CGFloat, scale: CGFloat) -> CGImage? {
		let alignedWidth = (size.width * scale).rounded(.up) / scale
		let alignedHeight = (size.height * scale).rounded(.up) / scale
		let alignedSize = CGSize(width: alignedWidth, height: alignedHeight)
		let width = max(1, Int((alignedSize.width * scale).rounded(.up)))
		let height = max(1, Int((alignedSize.height * scale).rounded(.up)))
		let colorSpace = CGColorSpaceCreateDeviceRGB()
		guard
			let context = CGContext(
				data: nil,
				width: width,
				height: height,
				bitsPerComponent: 8,
				bytesPerRow: 0,
				space: colorSpace,
				bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
			)
		else {
			return nil
		}
		context.scaleBy(x: scale, y: scale)
		context.interpolationQuality = .high
		context.setAllowsAntialiasing(true)
		context.setShouldAntialias(true)
		context.setFillColor(NSColor.clear.cgColor)
		context.fill(CGRect(origin: .zero, size: alignedSize))
		let rect = CGRect(origin: .zero, size: alignedSize)
		let maxRadius = min(alignedSize.width, alignedSize.height) / 2
		var adjustedRadius = min(cornerRadius, maxRadius)
		if abs(adjustedRadius - maxRadius) <= (0.5 / scale) {
			adjustedRadius = maxRadius
		}
		adjustedRadius = (adjustedRadius * scale).rounded(.toNearestOrAwayFromZero) / scale
		let path = CGPath(
			roundedRect: rect,
			cornerWidth: adjustedRadius,
			cornerHeight: adjustedRadius,
			transform: nil
		)
		context.addPath(path)
		context.setFillColor(NSColor.white.cgColor)
		context.fillPath()
		return context.makeImage()
	}

}
