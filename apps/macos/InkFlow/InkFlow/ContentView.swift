import AppKit
import SwiftUI

struct ContentView: View {
	@StateObject private var model = InkFlowViewModel()

	var body: some View {
		card
			.frame(minWidth: 700, minHeight: 72)
			.onExitCommand {
				NSApp.keyWindow?.orderOut(nil)
			}
	}

	@ViewBuilder
	private var card: some View {
		let content = HStack(alignment: .center, spacing: 9) {
			leadingBlock
			transcriptStrip
		}
		.padding(.horizontal, 10)
		.padding(.vertical, 3)
		.frame(maxWidth: 820, alignment: .center)

		let surface = content
			.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)

		if #available(macOS 26.0, *) {
			GlassEffectContainer(spacing: 5) {
				surface.glassEffect(.regular.tint(.white.opacity(0.12)), in: .rect(cornerRadius: 12))
			}
		} else {
			surface
				.background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 12))
		}
	}

	private var leadingBlock: some View {
		HStack(spacing: 8) {
			statusGlyph

			if let error = model.errorMessage {
				Text(error)
					.font(.system(size: 13, weight: .medium))
					.foregroundStyle(.red.opacity(0.85))
			}
		}
	}

	@ViewBuilder
	private var statusGlyph: some View {
		let button = Button(action: toggleListening) {
			ActivationOrbView(isActive: model.isListening)
		}
		.buttonStyle(.plain)
		button
	}

	private var transcriptStrip: some View {
		ZStack(alignment: .leading) {
			waveformBackdrop
			Text(model.transcript.isEmpty ? "Speak to dictate." : model.transcript)
				.font(.system(size: 16, weight: .medium))
				.foregroundStyle(.white)
				.lineLimit(1)
				.truncationMode(.tail)
				.shadow(color: .black.opacity(0.25), radius: 1)
				.mask(transcriptFadeMask)
		}
		.frame(maxWidth: .infinity, alignment: .leading)
		.padding(.vertical, 2)
		.padding(.horizontal, 6)
		.frame(height: 26)
		.background(transcriptBackground)
		.clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
	}

	private var transcriptFadeMask: some View {
		LinearGradient(
			colors: [.white, .white, .white.opacity(0.2)],
			startPoint: .leading,
			endPoint: .trailing
		)
	}

	@ViewBuilder
	private var waveformBackdrop: some View {
		WaveformView(levels: model.waveformLevels, isActive: model.isListening)
			.opacity(model.isListening ? 0.12 : 0.05)
			.blur(radius: 0.4)
			.frame(height: 16)
			.mask(waveformFadeMask)
	}

	private var waveformFadeMask: some View {
		LinearGradient(
			colors: [.clear, .white.opacity(0.9), .white.opacity(0.9), .clear],
			startPoint: .leading,
			endPoint: .trailing
		)
	}

	@ViewBuilder
	private var transcriptBackground: some View {
		if #available(macOS 26.0, *) {
			Color.clear.glassEffect(.regular.tint(.white.opacity(0.06)), in: .rect(cornerRadius: 8))
		} else {
			Color.black.opacity(0.16)
		}
	}

	private func toggleListening() {
		if model.isListening {
			model.stop()
		} else {
			model.start()
		}
	}
}

#Preview {
	ContentView()
}
