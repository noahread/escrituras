import AppKit
import EscriturasKit
import SwiftUI

/// The chapter being read
struct ReaderView: View {
    @Bindable var model: AppModel
    let chat: ChatModel
    @AppStorage(SettingsKey.readerFontSize) private var fontSize = ReaderFont.defaultSize
    var focus: FocusState<Pane?>.Binding

    var body: some View {
        if let location = model.location {
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: fontSize * 0.75) {
                        Text(location.title)
                            .font(.system(size: fontSize * 1.8, weight: .semibold, design: .serif))
                            .padding(.bottom, fontSize * 0.5)

                        ForEach(model.verses) { verse in
                            VerseRow(
                                verse: verse,
                                fontSize: fontSize,
                                isSelected: model.selectedVerse == verse.title,
                                isHighlighted: model.highlightedVerses?.contains(verse.number) ?? false,
                                isSaved: model.isSaved(verse),
                                note: model.note(for: verse)
                            )
                            .id(verse.title)
                            .contentShape(Rectangle())
                            .onTapGesture {
                                model.selectedVerse = verse.title
                                focus.wrappedValue = .reader
                            }
                            .contextMenu {
                                VerseActions(model: model, chat: chat, verse: verse)
                            }
                        }

                        ChapterFooter(model: model)
                            .padding(.top, fontSize)
                    }
                    .padding(.horizontal, 32)
                    .padding(.vertical, 24)
                    .frame(maxWidth: 760, alignment: .leading)
                    .frame(maxWidth: .infinity)
                }
                .focusable()
                .focused(focus, equals: .reader)
                .focusEffectDisabled()
                // ↑/↓ move verse to verse and on into the neighbouring chapters
                // (ColumnFocus handles ←)
                .onKeyPress(.downArrow) {
                    model.selectNextVerse()
                    return .handled
                }
                .onKeyPress(.upArrow) {
                    model.selectPreviousVerse()
                    return .handled
                }
                .onKeyPress(.home) {
                    model.selectFirstVerse()
                    return .handled
                }
                .onKeyPress(.end) {
                    model.selectLastVerse()
                    return .handled
                }
                // Nothing has keyboard focus when the window opens; start in the
                // reader so the arrow keys work right away
                .task {
                    try? await Task.sleep(for: .milliseconds(100))
                    if focus.wrappedValue == nil {
                        focus.wrappedValue = .reader
                    }
                }
                // Runs on appearance and whenever the target changes. Waiting a
                // moment lets the lazy stack lay out the verses first.
                .task(id: model.scrollTarget) {
                    guard let target = model.scrollTarget else { return }
                    try? await Task.sleep(for: .milliseconds(50))
                    proxy.scrollTo(target, anchor: .center)
                }
            }
        } else {
            ContentUnavailableView("Choose a Chapter", systemImage: "book")
        }
    }
}

struct VerseRow: View {
    let verse: Verse
    let fontSize: Double
    let isSelected: Bool
    let isHighlighted: Bool
    let isSaved: Bool
    let note: String?

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Text("\(verse.number)")
                .font(.system(size: fontSize * 0.7, weight: .medium))
                .foregroundStyle(.secondary)
                .monospacedDigit()
                .frame(minWidth: fontSize * 1.6, alignment: .trailing)

            VStack(alignment: .leading, spacing: 6) {
                Text(verse.text)
                    .font(.system(size: fontSize, design: .serif))
                    .lineSpacing(fontSize * 0.25)
                    .textSelection(.enabled)
                    .fixedSize(horizontal: false, vertical: true)

                if let note {
                    Label(note, systemImage: "note.text")
                        .font(.system(size: fontSize * 0.85))
                        .foregroundStyle(.secondary)
                        .padding(8)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(.yellow.opacity(0.12), in: RoundedRectangle(cornerRadius: 6))
                }
            }

            Spacer(minLength: 0)

            if isSaved {
                Image(systemName: "bookmark.fill")
                    .foregroundStyle(.tint)
                    .help("Saved")
                    .accessibilityLabel("Saved")
            }
        }
        .padding(.vertical, 4)
        .padding(.horizontal, 8)
        .background(background, in: RoundedRectangle(cornerRadius: 8))
        .accessibilityElement(children: .combine)
    }

    private var background: Color {
        if isSelected { return Color.accentColor.opacity(0.14) }
        if isHighlighted { return Color.accentColor.opacity(0.08) }
        return .clear
    }
}

/// Actions on a verse, shared by its context menu and the Verse menu
struct VerseActions: View {
    let model: AppModel
    let chat: ChatModel
    let verse: Verse

    var body: some View {
        Button("Copy Verse") {
            copyToPasteboard(AppModel.clipboardText(for: verse))
        }
        Button(model.isSaved(verse) ? "Remove from Saved Verses" : "Save Verse") {
            model.toggleSaved(verse)
        }
        Button(model.note(for: verse) == nil ? "Add Note…" : "Edit Note…") {
            model.noteEditorVerse = verse
        }
        Divider()
        Button("Ask About This Verse") {
            model.showInspector = true
            model.inspectorTab = .ask
            chat.ask(about: verse, reading: model)
        }
    }
}

func copyToPasteboard(_ text: String) {
    NSPasteboard.general.clearContents()
    NSPasteboard.general.setString(text, forType: .string)
}

/// Previous / next chapter links at the end of a chapter
struct ChapterFooter: View {
    let model: AppModel

    var body: some View {
        HStack {
            if let previous = model.previousChapter {
                Button {
                    model.show(previous)
                } label: {
                    Label(previous.title, systemImage: "chevron.backward")
                }
            }
            Spacer()
            if let next = model.nextChapter {
                Button {
                    model.show(next)
                } label: {
                    Label(next.title, systemImage: "chevron.forward")
                        .labelStyle(TrailingIconLabelStyle())
                }
            }
        }
        .buttonStyle(.link)
    }
}

struct TrailingIconLabelStyle: LabelStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack(spacing: 4) {
            configuration.title
            configuration.icon
        }
    }
}
