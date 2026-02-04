import SwiftUI

struct PanelHeaderTranscriptView: View {
	@ObservedObject var model: InkFlowViewModel

	var body: some View {
		ZStack(alignment: .leading) {
			PanelHeaderWaveformBackdrop(levels: model.waveformLevels, isActive: model.isListening)
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
}

private struct PanelHeaderWaveformBackdrop: View {
	let levels: [CGFloat]
	let isActive: Bool

	var body: some View {
		WaveformView(levels: levels, isActive: isActive)
			.opacity(
				isActive
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
				.clear,
			],
			startPoint: .leading,
			endPoint: .trailing
		)
	}
}
