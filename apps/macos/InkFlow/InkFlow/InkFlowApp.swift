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
	}
}

final class AppDelegate: NSObject, NSApplicationDelegate {
	private var panel: FloatingPanel?

	func applicationDidFinishLaunching(_ notification: Notification) {
		let panel = FloatingPanel(
			contentRect: NSRect(x: 0, y: 0, width: 680, height: 420),
			styleMask: [.borderless],
			backing: .buffered,
			defer: false
		)
		let cornerRadius: CGFloat = 24
		panel.level = .floating
		panel.isOpaque = false
		panel.backgroundColor = .clear
		panel.hasShadow = true
		panel.isMovableByWindowBackground = true
		panel.hidesOnDeactivate = false
		panel.isReleasedWhenClosed = false
		panel.titleVisibility = .hidden
		panel.titlebarAppearsTransparent = true
		panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]

		let hostingController = NSHostingController(rootView: ContentView())
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
		panel.center()
		panel.makeKeyAndOrderFront(nil)

		self.panel = panel
	}

	func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
		false
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
