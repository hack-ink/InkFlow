import SwiftUI

struct SettingsShortcutsSectionView: View {
	@State private var toggleDictation = ""
	@State private var pushToTalk = ""
	@State private var pasteLastTranscript = ""

	var body: some View {
		ScrollView {
			VStack(alignment: .leading, spacing: SettingsLayout.sectionSpacing) {
				SettingsGroupView(title: "Dictation") {
					SettingsShortcutRow(title: "Toggle dictation", value: $toggleDictation)
					SettingsShortcutRow(title: "Push-to-talk", value: $pushToTalk)
				}
				Divider()
				SettingsGroupView(title: "Output") {
					SettingsShortcutRow(title: "Paste last transcript", value: $pasteLastTranscript)
				}
				Divider()
				SettingsGroupView(title: "Defaults") {
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
