import SwiftUI

struct PanelHeaderView: View {
	@ObservedObject var model: InkFlowViewModel
	@ObservedObject var panelController: PanelController
	let appearance: AppearanceStyle

	var body: some View {
		headerRow
			.preferredColorScheme(appearance.preferredColorScheme)
			.tint(appearance.accentColor)
			.onExitCommand { panelController.handleExitCommand() }
	}

	private var headerRow: some View {
		HStack(alignment: .center, spacing: PanelHeaderLayout.rowSpacing) {
			leadingBlock
			transcriptStrip
			expandButton
		}
		.frame(maxWidth: .infinity, alignment: .leading)
		.frame(height: PanelHeaderLayout.rowHeight, alignment: .center)
		.textSelection(.disabled)
	}

	private var leadingBlock: some View {
		HStack(spacing: PanelHeaderLayout.leadingSpacing) {
			statusGlyph

			if let error = model.errorMessage {
				Text(error)
					.font(.system(size: 13, weight: .medium))
					.foregroundStyle(UIColors.errorText)
			}
		}
		.padding(.leading, PanelHeaderLayout.leadingInset)
	}

	private var statusGlyph: some View {
		Button(action: toggleListening) {
			ActivationOrbView(isActive: model.isListening)
		}
		.buttonStyle(.plain)
		.focusable(false)
	}

	private var transcriptStrip: some View {
		ZStack(alignment: .leading) {
			waveformBackdrop
			Text(model.transcript.isEmpty ? "Speak to dictate." : model.transcript)
				.font(.system(size: 16, weight: .medium))
				.foregroundStyle(.primary)
				.lineLimit(1)
				.truncationMode(.tail)
		}
		.frame(maxWidth: .infinity, alignment: .leading)
		.padding(.vertical, PanelHeaderLayout.transcriptVerticalPadding)
		.padding(.horizontal, PanelHeaderLayout.transcriptHorizontalPadding)
		.frame(height: PanelHeaderLayout.transcriptHeight)
		.textSelection(.disabled)
		.allowsHitTesting(false)
	}

	private var expandButton: some View {
		Button(action: panelController.toggleExpanded) {
			Image(systemName: panelController.isExpanded ? "chevron.up" : "chevron.down")
				.font(.system(size: 11, weight: .semibold))
				.foregroundStyle(.secondary)
				.frame(
					width: PanelHeaderLayout.expandButtonSize,
					height: PanelHeaderLayout.expandButtonSize)
		}
		.buttonStyle(.plain)
		.focusable(false)
		.animation(.easeInOut(duration: UIDuration.standard), value: panelController.isExpanded)
		.accessibilityLabel(panelController.isExpanded ? "Collapse panel" : "Expand panel")
	}

	private var waveformBackdrop: some View {
		WaveformView(levels: model.waveformLevels, isActive: model.isListening)
			.opacity(
				model.isListening
					? PanelHeaderLayout.waveformActiveOpacity
					: PanelHeaderLayout.waveformInactiveOpacity
			)
			.blur(radius: PanelHeaderLayout.waveformBlurRadius)
			.frame(height: PanelHeaderLayout.waveformHeight)
			.mask(waveformFadeMask)
	}

	private var waveformFadeMask: some View {
		LinearGradient(
			colors: [
				.clear,
				.white.opacity(PanelHeaderLayout.waveformMaskOpacity),
				.white.opacity(PanelHeaderLayout.waveformMaskOpacity),
				.clear
			],
			startPoint: .leading,
			endPoint: .trailing
		)
	}

	private func toggleListening() {
		if model.isListening {
			model.stop()
		} else {
			model.start()
		}
	}
}
