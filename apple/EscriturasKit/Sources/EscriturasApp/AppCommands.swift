import EscriturasKit
import SwiftUI

/// Menu bar commands. Shortcuts follow common Mac conventions:
/// ⌘[ / ⌘] for history (Safari, Finder), ⌘L to go to a location (Safari),
/// ⌘D to bookmark (Safari), ⌥⌘I for the inspector (Pages, Keynote),
/// ⌘F for the search field (Mail, Notes).
struct AppCommands: Commands {
    let launcher: AppLauncher

    var body: some Commands {
        CommandGroup(after: .pasteboard) {
            Divider()
            Button("Copy Verse") {
                if let verse = model?.selectedVerseValue {
                    copyToPasteboard(AppModel.clipboardText(for: verse))
                }
            }
            .keyboardShortcut("c", modifiers: [.command, .shift])
            .disabled(model?.selectedVerseValue == nil)
        }

        CommandGroup(after: .textEditing) {
            Button("Find…") { model?.focusSearch() }
                .keyboardShortcut("f", modifiers: .command)
                .disabled(model == nil)
        }

        CommandGroup(after: .sidebar) {
            Button(model?.showInspector == true ? "Hide Study Tools" : "Show Study Tools") {
                model?.showInspector.toggle()
            }
            .keyboardShortcut("i", modifiers: [.command, .option])
            .disabled(model == nil)

            Divider()

            Button("Bigger Text") { adjustFontSize(by: 1) }
                .keyboardShortcut("+", modifiers: .command)

            Button("Smaller Text") { adjustFontSize(by: -1) }
                .keyboardShortcut("-", modifiers: .command)

            Button("Actual Size") {
                UserDefaults.standard.set(ReaderFont.defaultSize, forKey: SettingsKey.readerFontSize)
            }
            .keyboardShortcut("0", modifiers: .command)
        }

        CommandMenu("Go") {
            Button("Back") { model?.goBack() }
                .keyboardShortcut("[", modifiers: .command)
                .disabled(model?.canGoBack != true)

            Button("Forward") { model?.goForward() }
                .keyboardShortcut("]", modifiers: .command)
                .disabled(model?.canGoForward != true)

            Divider()

            Button("Previous Chapter") { model?.goToPreviousChapter() }
                .keyboardShortcut(.leftArrow, modifiers: [.command, .option])
                .disabled(model?.previousChapter == nil)

            Button("Next Chapter") { model?.goToNextChapter() }
                .keyboardShortcut(.rightArrow, modifiers: [.command, .option])
                .disabled(model?.nextChapter == nil)

            Divider()

            Button("Go to Reference…") { model?.showGoToReference = true }
                .keyboardShortcut("l", modifiers: .command)
                .disabled(model == nil)
        }

        CommandMenu("Verse") {
            let verse = model?.selectedVerseValue
            let isSaved = verse.map { model?.isSaved($0) ?? false } ?? false
            let hasNote = verse.flatMap { model?.note(for: $0) } != nil

            Button(isSaved ? "Remove from Saved Verses" : "Save Verse") {
                if let verse { model?.toggleSaved(verse) }
            }
            .keyboardShortcut("d", modifiers: .command)
            .disabled(verse == nil)

            Button(hasNote ? "Edit Note…" : "Add Note…") {
                model?.noteEditorVerse = verse
            }
            .keyboardShortcut("n", modifiers: [.command, .option])
            .disabled(verse == nil)

            Divider()

            Button("Ask About This Verse") {
                guard let verse, let model, let chat = launcher.chat else { return }
                model.showInspector = true
                model.inspectorTab = .ask
                chat.ask(about: verse, reading: model)
            }
            .keyboardShortcut("a", modifiers: [.command, .shift])
            .disabled(verse == nil)
        }
    }

    private var model: AppModel? { launcher.model }

    /// The reader's text size, observed by the views through @AppStorage
    private func adjustFontSize(by step: Double) {
        let defaults = UserDefaults.standard
        let current = defaults.object(forKey: SettingsKey.readerFontSize) as? Double ?? ReaderFont.defaultSize
        let size = min(max(current + step, ReaderFont.range.lowerBound), ReaderFont.range.upperBound)
        defaults.set(size, forKey: SettingsKey.readerFontSize)
    }
}
