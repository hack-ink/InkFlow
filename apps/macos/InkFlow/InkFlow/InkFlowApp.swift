//
//  InkFlowApp.swift
//  InkFlow
//
//  Created by Xavier Lau on 1/16/26.
//

import AppKit
import SwiftUI

@main
struct InkFlowApp: App {
	@NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate

	var body: some Scene {
		Settings {
			EmptyView()
		}
		.commands {
			CommandGroup(replacing: .appSettings) {}
		}
	}
}

final class AppDelegate: NSObject, NSApplicationDelegate {
	private var panel: FloatingPanel?
	private var statusItem: NSStatusItem?
	private let panelController = PanelController()
	private let viewModel = InkFlowViewModel()
	private var hotkeyController: DictationHotkeyController?

	func applicationDidFinishLaunching(_ notification: Notification) {
		_ = ConfigStore.shared
		configureStatusItem()
		let panel = FloatingPanel(
			contentRect: NSRect(x: 0, y: 0, width: 720, height: 60),
			styleMask: [.borderless],
			backing: .buffered,
			defer: false
		)
		let cornerRadius: CGFloat = panel.frame.height / 2
		panel.level = .floating
		panel.isOpaque = false
		panel.backgroundColor = .clear
		panel.hasShadow = true
		panel.ignoresMouseEvents = false
		panel.isMovableByWindowBackground = true
		panel.hidesOnDeactivate = false
		panel.isReleasedWhenClosed = false
		panel.titleVisibility = .hidden
		panel.titlebarAppearsTransparent = true
		panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]

		let hostingController = PanelHostViewController(panelController: panelController, viewModel: viewModel)
		hostingController.view.wantsLayer = true
		hostingController.view.layer?.cornerRadius = cornerRadius
		hostingController.view.layer?.cornerCurve = .circular
		hostingController.view.layer?.masksToBounds = true
		panel.contentViewController = hostingController
		if let frameView = panel.contentView?.superview {
			frameView.wantsLayer = true
			frameView.layer?.cornerRadius = cornerRadius
			frameView.layer?.cornerCurve = .circular
			frameView.layer?.masksToBounds = true
		}
		panel.invalidateShadow()
		restorePanelPosition(panel)
		panel.makeKeyAndOrderFront(nil)

		self.panel = panel
		panelController.panel = panel
		NotificationCenter.default.addObserver(
			self, selector: #selector(panelDidResignKey(_:)), name: NSWindow.didResignKeyNotification,
			object: panel)
		NotificationCenter.default.addObserver(
			self, selector: #selector(panelDidMove(_:)), name: NSWindow.didMoveNotification, object: panel)
		panelController.syncPanelSize(animated: false)
		hotkeyController = DictationHotkeyController(
			viewModel: viewModel,
			panelController: panelController,
			config: ConfigStore.shared.current.dictation
		)
		hotkeyController?.start()
	}

	func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
		false
	}

	func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
		panelController.showPanel()
		return true
	}

	@objc func toggleSettings() {
		panelController.showPanel()
		panelController.toggleSettings()
	}

	@objc private func quitApp() {
		NSApp.terminate(nil)
	}

	@objc private func panelDidResignKey(_ notification: Notification) {
		guard let panel = notification.object as? NSPanel else {
			return
		}
		panel.orderOut(nil)
		panelController.setExpanded(false, animated: false)
		panelController.closeSettings()
	}

	@objc private func panelDidMove(_ notification: Notification) {
		guard let panel = notification.object as? NSPanel else {
			return
		}
		storePanelPosition(panel)
	}

	private func configureStatusItem() {
		let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
		if let button = item.button {
			let image = statusItemImage()
			image.isTemplate = true
			button.image = image
		}

		let menu = NSMenu()
		let quitItem = NSMenuItem(title: "Quit", action: #selector(quitApp), keyEquivalent: "q")
		quitItem.keyEquivalentModifierMask = [.command]
		quitItem.target = self
		menu.addItem(quitItem)

		item.menu = menu
		statusItem = item
	}

	private func statusItemImage() -> NSImage {
		if let image = NSImage(systemSymbolName: "mic.fill", accessibilityDescription: "InkFlow") {
			return image
		}
		if let image = NSImage(systemSymbolName: "waveform", accessibilityDescription: "InkFlow") {
			return image
		}
		return NSImage()
	}

	private func restorePanelPosition(_ panel: NSPanel) {
		let defaults = UserDefaults.standard
		let xKey = "panel.origin.x"
		let yKey = "panel.origin.y"
		guard defaults.object(forKey: xKey) != nil,
			let storedX = defaults.object(forKey: xKey) as? Double,
			let storedY = defaults.object(forKey: yKey) as? Double
		else {
			panel.center()
			return
		}
		var frame = panel.frame
		frame.origin = CGPoint(x: storedX, y: storedY)
		panel.setFrame(frame, display: false)
	}

	private func storePanelPosition(_ panel: NSPanel) {
		let defaults = UserDefaults.standard
		defaults.set(panel.frame.origin.x, forKey: "panel.origin.x")
		defaults.set(panel.frame.origin.y, forKey: "panel.origin.y")
	}
}

final class FloatingPanel: NSPanel {
	override var canBecomeKey: Bool {
		true
	}

	override var canBecomeMain: Bool {
		true
	}
}
