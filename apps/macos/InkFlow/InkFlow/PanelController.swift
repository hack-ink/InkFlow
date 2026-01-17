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
	private let animationDuration: TimeInterval = 0.32
	private let timingFunction = CAMediaTimingFunction(controlPoints: 0.22, 0.61, 0.36, 1.0)

	var expandedPanelHeight: CGFloat {
		expandedHeight
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
		var frame = panel.frame
		let delta = height - frame.height
		frame.size.height = height
		frame.size.width = panelWidth
		frame.origin.y -= delta
		if animated {
			NSAnimationContext.runAnimationGroup { context in
				context.duration = animationDuration
				context.timingFunction = timingFunction
				panel.animator().setFrame(frame, display: true)
			}
		} else {
			panel.setFrame(frame, display: true)
		}
	}
}
