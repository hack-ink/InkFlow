import AppKit
import SwiftUI

struct ContentView: View {
	@StateObject private var model = InkFlowViewModel()

	var body: some View {
		card
			.frame(minWidth: 560, minHeight: 360)
			.onExitCommand {
				NSApp.keyWindow?.orderOut(nil)
			}
	}

	@ViewBuilder
	private var card: some View {
		let content = VStack(alignment: .leading, spacing: 16) {
			header
			transcriptBlock
			controls
		}
		.frame(maxWidth: 640)

		let surface = content
			.padding(24)
			.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)

		if #available(macOS 26.0, *) {
			GlassEffectContainer(spacing: 24) {
				surface
					.glassEffect(.regular.tint(.white.opacity(0.12)), in: .rect(cornerRadius: 24))
			}
		} else {
			surface
				.background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 24))
		}
	}

	private var header: some View {
		HStack(spacing: 16) {
			Image(systemName: model.isListening ? "waveform.circle.fill" : "mic.circle.fill")
				.font(.system(size: 32, weight: .semibold))
				.foregroundStyle(.white)

			VStack(alignment: .leading, spacing: 4) {
				Text("InkFlow")
					.font(.title2.weight(.semibold))
					.foregroundStyle(.white)

				Text(model.status)
					.font(.subheadline)
					.foregroundStyle(.white.opacity(0.7))
			}

			Spacer()

			statusChip
		}
	}

	@ViewBuilder
	private var statusChip: some View {
		let text = model.isListening ? "Listening" : "Idle"
		let chip = Text(text)
			.font(.caption.weight(.semibold))
			.foregroundStyle(.white)
			.padding(.horizontal, 12)
			.padding(.vertical, 6)

		if #available(macOS 26.0, *) {
			chip.glassEffect(.regular.tint(.white.opacity(0.12)), in: .capsule)
		} else {
			chip.background(.ultraThinMaterial, in: Capsule())
		}
	}

	private var transcriptBlock: some View {
		VStack(alignment: .leading, spacing: 8) {
			Text("Live Transcript")
				.font(.subheadline.weight(.semibold))
				.foregroundStyle(.white.opacity(0.7))

			ScrollView {
				Text(model.transcript.isEmpty ? "Say something to begin." : model.transcript)
					.font(.body)
					.foregroundStyle(.white)
					.frame(maxWidth: .infinity, alignment: .leading)
			}
			.frame(minHeight: 140)
			.padding(16)
			.background(transcriptBackground)
			.clipShape(RoundedRectangle(cornerRadius: 16))

			if let error = model.errorMessage {
				Text(error)
					.font(.caption)
					.foregroundStyle(.red.opacity(0.85))
			}
		}
	}

	@ViewBuilder
	private var transcriptBackground: some View {
		if #available(macOS 26.0, *) {
			Color.clear.glassEffect(.regular.tint(.white.opacity(0.08)), in: .rect(cornerRadius: 16))
		} else {
			Color.black.opacity(0.2)
		}
	}

	private var controls: some View {
		HStack(spacing: 12) {
			styledButton(model.isListening ? "Stop" : "Start", prominent: !model.isListening) {
				if model.isListening {
					model.stop()
				} else {
					model.start()
				}
			}

			styledButton("Clear", prominent: false) {
				model.clear()
			}
		}
		.font(.headline)
		.foregroundStyle(.white)
	}

	@ViewBuilder
	private func styledButton(
		_ title: String,
		prominent: Bool,
		action: @escaping () -> Void
	) -> some View {
		let button = Button(title, action: action)
			.padding(.vertical, 10)
			.padding(.horizontal, 16)
			.buttonStyle(.plain)
		if #available(macOS 26.0, *) {
			let tint = prominent ? Color.white.opacity(0.2) : Color.white.opacity(0.1)
			button
				.glassEffect(.regular.tint(tint).interactive(), in: .rect(cornerRadius: 12))
		} else {
			let fill = prominent ? Color.white.opacity(0.18) : Color.white.opacity(0.1)
			button
				.background(fill, in: RoundedRectangle(cornerRadius: 12))
				.overlay(RoundedRectangle(cornerRadius: 12).stroke(Color.white.opacity(0.2)))
		}
	}
}

#Preview {
	ContentView()
}
