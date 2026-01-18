import AppKit
import Combine
import SwiftUI

final class PanelHostViewController: NSViewController {
	private let panelController: PanelController
	private let viewModel: InkFlowViewModel
	private var cancellables: Set<AnyCancellable> = []

	private let backgroundHost: NSHostingView<AppearanceReader<PanelBackgroundView>>
	private let headerHost: NSHostingView<AppearanceReader<PanelHeaderView>>
	private let expandedHost: NSHostingView<AppearanceReader<PanelExpandedView>>

	private var headerTopConstraint: NSLayoutConstraint?
	private var expandedHeightConstraint: NSLayoutConstraint?

	init(panelController: PanelController, viewModel: InkFlowViewModel) {
		self.panelController = panelController
		self.viewModel = viewModel
		self.backgroundHost = NSHostingView(
			rootView: AppearanceReader { appearance in
				PanelBackgroundView(panelController: panelController, appearance: appearance)
			}
		)
		self.headerHost = NSHostingView(
			rootView: AppearanceReader { appearance in
				PanelHeaderView(model: viewModel, panelController: panelController, appearance: appearance)
			}
		)
		self.expandedHost = NSHostingView(
			rootView: AppearanceReader { appearance in
				PanelExpandedView(panelController: panelController, appearance: appearance)
			}
		)
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
		let padding = UIPanelLayout.padding
		let spacing = UIPanelLayout.headerSpacing
		let headerHeight = UIPanelLayout.headerHeight

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

		headerTopConstraint = headerHost.topAnchor.constraint(
			equalTo: view.topAnchor,
			constant: headerTopInset(forExpanded: panelController.isExpanded)
		)

		NSLayoutConstraint.activate([
			headerTopConstraint,
			headerHost.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: padding),
			headerHost.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -padding),
			headerHost.heightAnchor.constraint(equalToConstant: headerHeight),

			expandedHost.topAnchor.constraint(equalTo: headerHost.bottomAnchor, constant: spacing),
			expandedHost.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: padding),
			expandedHost.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -padding)
		].compactMap { $0 })

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
		headerTopConstraint?.constant = headerTopInset(forExpanded: expanded)
		expandedHost.isHidden = false
		expandedHost.alphaValue = expanded ? 1.0 : 0.0
		expandedHeightConstraint?.constant = expanded ? expandedContentHeight : 0
		view.layoutSubtreeIfNeeded()
		if !expanded {
			expandedHost.isHidden = true
		}
	}

	private var expandedContentHeight: CGFloat {
		let padding = UIPanelLayout.padding
		let spacing = UIPanelLayout.headerSpacing
		let headerHeight = UIPanelLayout.headerHeight
		let targetHeight = panelController.expandedPanelHeight
		return max(0, targetHeight - (padding * 2 + spacing + headerHeight))
	}

	private func headerTopInset(forExpanded expanded: Bool) -> CGFloat {
		if expanded {
			return UIPanelLayout.padding
		}
		let collapsedHeight = panelController.collapsedPanelHeight
		let headerHeight = UIPanelLayout.headerHeight
		return max(0, (collapsedHeight - headerHeight) / 2)
	}
}
