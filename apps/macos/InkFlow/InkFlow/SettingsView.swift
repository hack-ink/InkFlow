import AppKit
import AVFoundation
import Combine
import SwiftUI

enum SettingsSection: String, CaseIterable, Identifiable {
	case appearance
	case microphone
	case shortcuts

	var id: String { rawValue }

	var title: String {
		switch self {
		case .appearance:
			return "Appearance"
		case .microphone:
			return "Microphone"
		case .shortcuts:
			return "Shortcuts"
		}
	}
}

struct SettingsView: View {
	@AppStorage("settings.selectedSection") private var selectedSectionRaw = SettingsSection.appearance.rawValue
	@AppStorage("appearance.theme") private var themeRaw = ThemePreference.system.rawValue
	@AppStorage("appearance.accent") private var accentRaw = AccentOption.sky.rawValue
	@AppStorage("appearance.glassIntensity") private var glassIntensityRaw = GlassIntensity.standard.rawValue
	@AppStorage("appearance.windowTranslucency") private var isWindowTranslucent = true

	private var appearance: AppearanceStyle {
		AppearanceStyle(
			theme: AppearanceStyle.theme(from: themeRaw),
			accent: AppearanceStyle.accent(from: accentRaw),
			glassIntensity: AppearanceStyle.glassIntensity(from: glassIntensityRaw),
			isTranslucent: isWindowTranslucent
		)
	}

	var body: some View {
		HStack(spacing: SettingsLayout.rootSpacing) {
			sidebar
			Divider()
			detailView
		}
		.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
		.preferredColorScheme(appearance.preferredColorScheme)
		.tint(appearance.accentColor)
	}

	private var selectedSection: SettingsSection {
		SettingsSection(rawValue: selectedSectionRaw) ?? .appearance
	}

	private var sidebar: some View {
		VStack(alignment: .leading, spacing: SettingsLayout.sidebarSpacing) {
			ForEach(SettingsSection.allCases) { section in
				SidebarButton(title: section.title, isSelected: selectedSection == section) {
					withAnimation(.easeInOut(duration: UIDuration.selectionChange)) {
						selectedSectionRaw = section.rawValue
					}
				}
			}
		}
		.frame(width: SettingsLayout.sidebarWidth, alignment: .leading)
	}

	@ViewBuilder
	private var detailView: some View {
		Group {
			switch selectedSection {
			case .appearance:
				AppearanceSettingsView()
			case .microphone:
				MicrophoneSettingsView()
			case .shortcuts:
				ShortcutsSettingsView()
			}
		}
		.id(selectedSection)
		.transition(.opacity)
		.animation(.easeInOut(duration: UIDuration.standard), value: selectedSectionRaw)
		.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
	}
}

private struct SidebarButton: View {
	let title: String
	let isSelected: Bool
	let action: () -> Void
	@State private var isHovered = false

	var body: some View {
		Button(action: action) {
			Text(title)
				.font(.system(size: 12, weight: .medium))
				.foregroundStyle(isSelected ? Color.primary : Color.secondary)
				.frame(maxWidth: .infinity, alignment: .leading)
				.padding(.vertical, SettingsLayout.sidebarItemVerticalPadding)
				.padding(.horizontal, SettingsLayout.sidebarItemHorizontalPadding)
				.background(background)
				.clipShape(RoundedRectangle(cornerRadius: UICornerRadius.small, style: .continuous))
		}
		.buttonStyle(.plain)
		.onHover { hovering in
			withAnimation(.easeInOut(duration: UIDuration.hoverFade)) {
				isHovered = hovering
			}
		}
	}

	@ViewBuilder
	private var background: some View {
		if isSelected {
			RoundedRectangle(cornerRadius: UICornerRadius.small, style: .continuous)
				.fill(UIColors.sidebarSelectedBackground)
		} else if isHovered {
			RoundedRectangle(cornerRadius: UICornerRadius.small, style: .continuous)
				.fill(UIColors.sidebarHoverBackground)
		} else {
			Color.clear
		}
	}
}

private struct AppearanceSettingsView: View {
	@AppStorage("appearance.theme") private var themeRaw = ThemePreference.system.rawValue
	@AppStorage("appearance.accent") private var accentRaw = AccentOption.sky.rawValue
	@AppStorage("appearance.glassIntensity") private var glassIntensityRaw = GlassIntensity.standard.rawValue
	@AppStorage("appearance.windowTranslucency") private var isWindowTranslucent = true
#if DEBUG
	@AppStorage("debug.showOrbFrame") private var showOrbFrame = false
#endif

	var body: some View {
		ScrollView {
			VStack(alignment: .leading, spacing: SettingsLayout.sectionSpacing) {
				SettingsGroup(title: "Theme") {
					Picker("Theme", selection: themeBinding) {
						ForEach(ThemePreference.allCases) { option in
							Text(option.title)
								.tag(option)
						}
					}
					.pickerStyle(.segmented)
				}
				Divider()
				SettingsGroup(title: "Accent Color") {
					LazyVGrid(
						columns: Array(
							repeating: GridItem(.fixed(SettingsLayout.accentGridItemSize), spacing: SettingsLayout.accentGridSpacing),
							count: SettingsLayout.accentGridColumns
						),
						spacing: SettingsLayout.accentGridSpacing
					) {
						ForEach(AccentOption.allCases) { option in
							Button {
								withAnimation(.easeInOut(duration: UIDuration.selectionChange)) {
									accentRaw = option.rawValue
								}
							} label: {
								Circle()
									.fill(option.color)
									.frame(width: UISize.accentSwatch, height: UISize.accentSwatch)
									.overlay(selectionRing(for: option))
							}
							.buttonStyle(.plain)
							.accessibilityLabel(option.title)
						}
					}
				}
				Divider()
				SettingsGroup(title: "Window and Glass") {
					VStack(alignment: .leading, spacing: SettingsLayout.groupSpacing) {
						Picker("Glass intensity", selection: glassIntensityBinding) {
							ForEach(GlassIntensity.allCases) { option in
								Text(option.title)
									.tag(option)
							}
						}
						.pickerStyle(.segmented)

						Toggle("Window translucency", isOn: $isWindowTranslucent)
					}
				}
#if DEBUG
				Divider()
				SettingsGroup(title: "Debug") {
					Toggle("Show orb frame", isOn: $showOrbFrame)
				}
#endif
			}
			.padding(.vertical, SettingsLayout.scrollVerticalPadding)
		}
	}

	private var themeBinding: Binding<ThemePreference> {
		Binding(
			get: { AppearanceStyle.theme(from: themeRaw) },
			set: { themeRaw = $0.rawValue }
		)
	}

	private var glassIntensityBinding: Binding<GlassIntensity> {
		Binding(
			get: { AppearanceStyle.glassIntensity(from: glassIntensityRaw) },
			set: { glassIntensityRaw = $0.rawValue }
		)
	}

	@ViewBuilder
	private func selectionRing(for option: AccentOption) -> some View {
		if AppearanceStyle.accent(from: accentRaw) == option {
			Circle()
				.strokeBorder(UIColors.selectionRing, lineWidth: SettingsLayout.selectionRingLineWidth)
		}
	}
}

private struct MicrophoneSettingsView: View {
	@AppStorage("microphone.inputDeviceID") private var selectedDeviceID = ""
	@StateObject private var testModel = MicrophoneTestModel()
	@State private var devices: [AudioInputDevice] = []

	var body: some View {
		ScrollView {
			VStack(alignment: .leading, spacing: SettingsLayout.sectionSpacing) {
				SettingsGroup(title: "Input Device") {
					if devices.isEmpty {
						Text("No input devices found.")
							.foregroundStyle(.secondary)
					} else {
						Picker("Input device", selection: $selectedDeviceID) {
							ForEach(devices) { device in
								Text(device.name)
									.tag(device.id)
							}
						}
						.pickerStyle(.menu)
					}
				}
				Divider()
				SettingsGroup(title: "Input Level") {
					VStack(alignment: .leading, spacing: SettingsLayout.inlineSpacing) {
						LevelMeterView(level: testModel.level, isActive: testModel.isTesting)
						if testModel.isTesting {
							Text("Listening...")
								.font(.caption)
								.foregroundStyle(.secondary)
						}
						if let error = testModel.errorMessage {
							Text(error)
								.font(.caption)
								.foregroundStyle(.secondary)
						}
					}
				}
				Divider()
				SettingsGroup(title: "Test Input") {
					Button(testModel.isTesting ? "Stop" : "Test Input") {
						testModel.toggleTest()
					}
					.buttonStyle(.bordered)
					.controlSize(.small)
				}
			}
			.padding(.vertical, SettingsLayout.scrollVerticalPadding)
		}
		.onAppear {
			devices = AudioInputDevice.available()
			if selectedDeviceID.isEmpty || !devices.contains(where: { $0.id == selectedDeviceID }) {
				selectedDeviceID = devices.first?.id ?? ""
			}
		}
	}
}

private struct ShortcutsSettingsView: View {
	@State private var toggleDictation = ""
	@State private var pushToTalk = ""
	@State private var pasteLastTranscript = ""

	var body: some View {
		ScrollView {
			VStack(alignment: .leading, spacing: SettingsLayout.sectionSpacing) {
				SettingsGroup(title: "Dictation") {
					ShortcutRow(title: "Toggle dictation", value: $toggleDictation)
					ShortcutRow(title: "Push-to-talk", value: $pushToTalk)
				}
				Divider()
				SettingsGroup(title: "Output") {
					ShortcutRow(title: "Paste last transcript", value: $pasteLastTranscript)
				}
				Divider()
				SettingsGroup(title: "Defaults") {
					Button("Reset to defaults") {
						toggleDictation = ""
						pushToTalk = ""
						pasteLastTranscript = ""
					}
					.buttonStyle(.bordered)
					.controlSize(.small)
				}
			}
			.padding(.vertical, SettingsLayout.scrollVerticalPadding)
		}
	}
}

private struct SettingsGroup<Content: View>: View {
	let title: String
	let content: Content

	init(title: String, @ViewBuilder content: () -> Content) {
		self.title = title
		self.content = content()
	}

	var body: some View {
		VStack(alignment: .leading, spacing: SettingsLayout.groupSpacing) {
			Text(title)
				.font(.caption.weight(.semibold))
				.foregroundStyle(.secondary)
			content
		}
		.frame(maxWidth: .infinity, alignment: .leading)
	}
}

private struct ShortcutRow: View {
	let title: String
	@Binding var value: String

	var body: some View {
		HStack {
			Text(title)
				.frame(width: SettingsLayout.shortcutLabelWidth, alignment: .leading)
			TextField("", text: $value)
				.textFieldStyle(.plain)
				.font(.system(size: 13, weight: .medium, design: .monospaced))
				.padding(.vertical, SettingsLayout.shortcutFieldVerticalPadding)
				.padding(.horizontal, SettingsLayout.shortcutFieldHorizontalPadding)
				.background(
					RoundedRectangle(cornerRadius: UICornerRadius.small, style: .continuous)
						.fill(UIColors.shortcutFieldBackground)
				)
		}
	}
}

private struct LevelMeterView: View {
	let level: CGFloat
	let isActive: Bool

	var body: some View {
		GeometryReader { proxy in
			let width = proxy.size.width
			let height = proxy.size.height
			let filled = max(min(level, 1), 0) * width

			ZStack(alignment: .leading) {
				RoundedRectangle(cornerRadius: height / 2, style: .continuous)
					.fill(UIColors.levelMeterTrack)
				RoundedRectangle(cornerRadius: height / 2, style: .continuous)
					.fill(isActive ? UIColors.levelMeterActiveFill : UIColors.levelMeterInactiveFill)
					.frame(width: filled)
			}
		}
		.frame(height: UISize.levelMeterHeight)
		.animation(.easeOut(duration: UIDuration.meterLevel), value: level)
		.animation(.easeInOut(duration: UIDuration.standard), value: isActive)
	}
}

private enum SettingsLayout {
	static let rootSpacing: CGFloat = UISpacing.xLarge
	static let sidebarSpacing: CGFloat = UISpacing.small
	static let sidebarWidth: CGFloat = 150
	static let sidebarItemVerticalPadding: CGFloat = UISpacing.small
	static let sidebarItemHorizontalPadding: CGFloat = UISpacing.medium
	static let sectionSpacing: CGFloat = UISpacing.xLarge
	static let groupSpacing: CGFloat = 10
	static let inlineSpacing: CGFloat = UISpacing.small
	static let scrollVerticalPadding: CGFloat = UISpacing.medium
	static let accentGridItemSize: CGFloat = 26
	static let accentGridSpacing: CGFloat = 10
	static let accentGridColumns: Int = 6
	static let shortcutLabelWidth: CGFloat = 170
	static let shortcutFieldVerticalPadding: CGFloat = 5
	static let shortcutFieldHorizontalPadding: CGFloat = UISpacing.medium
	static let selectionRingLineWidth: CGFloat = 2
}

private struct AudioInputDevice: Identifiable {
	let id: String
	let name: String

	static func available() -> [AudioInputDevice] {
		let session = AVCaptureDevice.DiscoverySession(
			deviceTypes: [.microphone, .external],
			mediaType: .audio,
			position: .unspecified
		)
		return session.devices.map { AudioInputDevice(id: $0.uniqueID, name: $0.localizedName) }
	}
}

private final class MicrophoneTestModel: ObservableObject {
	@Published var level: CGFloat = 0
	@Published var isTesting = false
	@Published var errorMessage: String?

	private let engine = AVAudioEngine()
	private let queue = DispatchQueue(label: "inkflow.microphone.test", qos: .userInitiated)
	private var lastUpdate: TimeInterval = 0
	private var lastLevel: CGFloat = 0
	private var stopWorkItem: DispatchWorkItem?

	func toggleTest() {
		if isTesting {
			stop()
		} else {
			start()
		}
	}

	private func start() {
		guard !isTesting else {
			return
		}

		errorMessage = nil
		requestMicrophoneAccess { [weak self] granted in
			guard let self else {
				return
			}
			guard granted else {
				self.errorMessage = "Enable microphone access in System Settings."
				return
			}
			self.beginCapture()
		}
	}

	private func beginCapture() {
		queue.async { [weak self] in
			guard let self else {
				return
			}

			do {
				try self.configureEngine()
				try self.engine.start()
				DispatchQueue.main.async {
					self.isTesting = true
				}
				self.scheduleAutoStop()
			} catch {
				DispatchQueue.main.async {
					self.errorMessage = "Microphone test failed."
					self.isTesting = false
				}
				self.engine.stop()
			}
		}
	}

	private func configureEngine() throws {
		engine.stop()
		engine.inputNode.removeTap(onBus: 0)

		let format = engine.inputNode.outputFormat(forBus: 0)
		engine.inputNode.installTap(onBus: 0, bufferSize: 512, format: format) { [weak self] buffer, _ in
			guard let self else {
				return
			}
			let level = self.calculateLevel(from: buffer)
			self.updateLevel(level)
		}
	}

	private func calculateLevel(from buffer: AVAudioPCMBuffer) -> CGFloat {
		guard let channelData = buffer.floatChannelData?.pointee else {
			return 0
		}
		let frameLength = Int(buffer.frameLength)
		if frameLength == 0 {
			return 0
		}
		var sum: Float = 0
		for index in 0..<frameLength {
			let sample = channelData[index]
			sum += sample * sample
		}
		let rms = sqrt(sum / Float(frameLength))
		let scaled = min(max(rms * 8, 0), 1)
		return CGFloat(scaled)
	}

	private func updateLevel(_ level: CGFloat) {
		let now = CACurrentMediaTime()
		if now - lastUpdate < 0.05 {
			lastLevel = level
			return
		}
		lastUpdate = now
		let averaged = (lastLevel + level) / 2
		lastLevel = level
		DispatchQueue.main.async {
			self.level = averaged
		}
	}

	private func scheduleAutoStop() {
		stopWorkItem?.cancel()
		let workItem = DispatchWorkItem { [weak self] in
			self?.stop()
		}
		stopWorkItem = workItem
		DispatchQueue.main.asyncAfter(deadline: .now() + 6, execute: workItem)
	}

	private func stop() {
		stopWorkItem?.cancel()
		stopWorkItem = nil
		engine.stop()
		engine.inputNode.removeTap(onBus: 0)
		DispatchQueue.main.async {
			self.isTesting = false
			self.level = 0
		}
	}

	private func requestMicrophoneAccess(_ completion: @escaping (Bool) -> Void) {
		switch AVCaptureDevice.authorizationStatus(for: .audio) {
		case .authorized:
			completion(true)
		case .notDetermined:
			AVCaptureDevice.requestAccess(for: .audio) { granted in
				DispatchQueue.main.async {
					completion(granted)
				}
			}
		default:
			completion(false)
		}
	}
}
