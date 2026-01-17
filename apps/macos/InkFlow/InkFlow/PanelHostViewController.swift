import AppKit
import Combine
import SwiftUI

final class PanelHostViewController: NSViewController {
	private let panelController: PanelController
	private let viewModel: InkFlowViewModel
	private var cancellables: Set<AnyCancellable> = []

	private let backgroundHost: NSHostingView<PanelBackgroundView>
	private let headerHost: NSHostingView<PanelHeaderView>
	private let expandedHost: NSHostingView<PanelExpandedView>

	private var expandedHeightConstraint: NSLayoutConstraint?

	init(panelController: PanelController, viewModel: InkFlowViewModel) {
		self.panelController = panelController
		self.viewModel = viewModel
		self.backgroundHost = NSHostingView(rootView: PanelBackgroundView(panelController: panelController))
		self.headerHost = NSHostingView(rootView: PanelHeaderView(model: viewModel, panelController: panelController))
		self.expandedHost = NSHostingView(rootView: PanelExpandedView(panelController: panelController))
		super.init(nibName: nil, bundle: nil)
	}

	required init?(coder: NSCoder) {
		return nil
	}

	override func loadView() {
		view = NSView()
		view.wantsLayer = true
		setupViews()
		bindState()
		applyExpandedState(panelController.isExpanded)
	}

	private func setupViews() {
		let padding: CGFloat = 8
		let spacing: CGFloat = 6
		let headerHeight: CGFloat = 40

		backgroundHost.translatesAutoresizingMaskIntoConstraints = false
		headerHost.translatesAutoresizingMaskIntoConstraints = false
		expandedHost.translatesAutoresizingMaskIntoConstraints = false

		view.addSubview(backgroundHost)
		view.addSubview(headerHost)
		view.addSubview(expandedHost)
		backgroundHost.wantsLayer = true
		backgroundHost.layer?.backgroundColor = NSColor.clear.cgColor

		NSLayoutConstraint.activate([
			backgroundHost.topAnchor.constraint(equalTo: view.topAnchor),
			backgroundHost.leadingAnchor.constraint(equalTo: view.leadingAnchor),
			backgroundHost.trailingAnchor.constraint(equalTo: view.trailingAnchor),
			backgroundHost.bottomAnchor.constraint(equalTo: view.bottomAnchor),
		])

		NSLayoutConstraint.activate([
			headerHost.topAnchor.constraint(equalTo: view.topAnchor, constant: padding),
			headerHost.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: padding),
			headerHost.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -padding),
			headerHost.heightAnchor.constraint(equalToConstant: headerHeight),

			expandedHost.topAnchor.constraint(equalTo: headerHost.bottomAnchor, constant: spacing),
			expandedHost.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: padding),
			expandedHost.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -padding)
		])

		expandedHeightConstraint = expandedHost.heightAnchor.constraint(equalToConstant: 0)
		expandedHeightConstraint?.isActive = true
	}

	private func bindState() {
		panelController.$isExpanded
			.receive(on: RunLoop.main)
			.sink { [weak self] expanded in
				self?.applyExpandedState(expanded)
			}
			.store(in: &cancellables)
	}

	private func applyExpandedState(_ expanded: Bool) {
		expandedHost.isHidden = false
		expandedHost.alphaValue = expanded ? 1.0 : 0.0
		expandedHeightConstraint?.constant = expanded ? expandedContentHeight : 0
		view.layoutSubtreeIfNeeded()
		if !expanded {
			expandedHost.isHidden = true
		}
	}

	private var expandedContentHeight: CGFloat {
		let padding: CGFloat = 8
		let spacing: CGFloat = 6
		let headerHeight: CGFloat = 40
		let targetHeight = panelController.expandedPanelHeight
		return max(0, targetHeight - (padding * 2 + spacing + headerHeight))
	}
}
