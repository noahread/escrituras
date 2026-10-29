import EscriturasKit
import SwiftUI

/// Saved verses, notes and AI chat, beside the reader
struct InspectorView: View {
    @Bindable var model: AppModel
    let chat: ChatModel

    var body: some View {
        Group {
            switch model.inspectorTab {
            case .study:
                StudyPanelView(model: model)
            case .ask:
                ChatView(model: model, chat: chat)
            }
        }
        // As in Xcode and Pages, the inspector's tabs sit in its own part of
        // the toolbar. That also gives the toolbar a divider at the
        // inspector's edge, so the window's items stay over the reader.
        .toolbar {
            ToolbarItem(placement: .principal) {
                Picker("Inspector", selection: $model.inspectorTab) {
                    Text("Study").tag(AppModel.InspectorTab.study)
                    Text("Ask").tag(AppModel.InspectorTab.ask)
                }
                .pickerStyle(.segmented)
                .labelsHidden()
            }
        }
    }
}

/// Saved verses and notes
struct StudyPanelView: View {
    let model: AppModel

    var body: some View {
        List {
            Section("Saved Verses") {
                if model.savedVerses.isEmpty {
                    Text("Save verses with ⌘D or the verse's context menu. Saved verses are included when you ask questions.")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                }
                ForEach(model.savedVerses) { verse in
                    Button {
                        model.show(verse: verse)
                    } label: {
                        VerseSummary(title: verse.title, detail: verse.text)
                    }
                    .buttonStyle(.plain)
                    .contextMenu {
                        Button("Remove from Saved Verses") { model.toggleSaved(verse) }
                    }
                }
            }

            Section("Notes") {
                let noted = model.notedVerses
                if noted.isEmpty {
                    Text("Add a note to a verse with ⌥⌘N or the verse's context menu.")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                }
                ForEach(noted) { item in
                    Button {
                        model.show(verse: item.verse)
                    } label: {
                        VerseSummary(title: item.verse.title, detail: item.note.body)
                    }
                    .buttonStyle(.plain)
                    .contextMenu {
                        Button("Edit Note…") { model.noteEditorVerse = item.verse }
                        Button("Delete Note", role: .destructive) { model.setNote("", for: item.verse) }
                    }
                }
            }
        }
        // On the inspector's own background, as Pages lists its settings
        .scrollContentBackground(.hidden)
    }
}

struct VerseSummary: View {
    let title: String
    let detail: String

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title).font(.headline)
            Text(detail)
                .font(.callout)
                .foregroundStyle(.secondary)
                .lineLimit(3)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .contentShape(Rectangle())
    }
}

/// The AI conversation
struct ChatView: View {
    let model: AppModel
    @Bindable var chat: ChatModel
    @AppStorage(SettingsKey.provider) private var providerName = AiProvider.claude.storageName
    @FocusState private var inputFocused: Bool

    private var settings: AISettings { .current() }

    var body: some View {
        VStack(spacing: 0) {
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 14) {
                        if chat.entries.isEmpty && !chat.isResponding {
                            Text("Ask a question about what you're reading. Answers include references you can click to open.")
                                .foregroundStyle(.secondary)
                                .padding(.top, 8)
                        }
                        ForEach(chat.entries) { entry in
                            ChatEntryView(entry: entry, model: model)
                        }
                        if chat.isResponding {
                            if chat.streamingReply.isEmpty {
                                ProgressView()
                                    .controlSize(.small)
                            } else {
                                Text(markdown: chat.streamingReply)
                                    .textSelection(.enabled)
                            }
                        }
                        Color.clear.frame(height: 1).id("bottom")
                    }
                    .padding(12)
                }
                .onChange(of: chat.streamingReply) {
                    proxy.scrollTo("bottom", anchor: .bottom)
                }
                .onChange(of: chat.entries.count) {
                    proxy.scrollTo("bottom", anchor: .bottom)
                }
            }

            Divider()

            VStack(alignment: .leading, spacing: 6) {
                HStack(alignment: .bottom) {
                    TextField("Ask about the scriptures", text: $chat.draft, axis: .vertical)
                        .textFieldStyle(.plain)
                        .lineLimit(1...6)
                        .focused($inputFocused)
                        .onSubmit { chat.send(reading: model) }
                    Button {
                        chat.send(reading: model)
                    } label: {
                        Label("Send Question", systemImage: "arrow.up.circle.fill")
                            .labelStyle(.iconOnly)
                            .font(.title2)
                    }
                    .buttonStyle(.borderless)
                    .disabled(chat.draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || chat.isResponding)
                    .help("Send")
                    .keyboardShortcut(.return, modifiers: .command)
                }
                HStack {
                    Text("\(settings.provider.displayName) · \(settings.model)")
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .layoutPriority(-1)
                    Spacer(minLength: 4)
                    if !chat.entries.isEmpty {
                        Button("Clear") { chat.clear() }
                            .buttonStyle(.link)
                            .disabled(chat.isResponding)
                            .help("Clear conversation")
                    }
                    SettingsLink {
                        Text("Settings…")
                    }
                    .buttonStyle(.link)
                }
                .font(.caption)
                .foregroundStyle(.secondary)
            }
            .padding(10)
        }
        .onAppear { inputFocused = true }
        .id(providerName) // refresh the provider line when settings change
    }
}

struct ChatEntryView: View {
    let entry: ChatEntry
    let model: AppModel

    var body: some View {
        switch entry.role {
        case .user:
            Text(entry.text)
                .padding(8)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(.quaternary, in: RoundedRectangle(cornerRadius: 8))
                .textSelection(.enabled)
        case .assistant:
            VStack(alignment: .leading, spacing: 8) {
                Text(markdown: entry.text)
                    .textSelection(.enabled)
                if !entry.references.isEmpty {
                    FlowingReferences(references: entry.references, model: model)
                }
            }
        case .error:
            Label(entry.text, systemImage: "exclamationmark.triangle")
                .foregroundStyle(.red)
                .font(.callout)
        }
    }
}

/// Buttons that open the references mentioned in a reply
struct FlowingReferences: View {
    let references: [ScriptureRef]
    let model: AppModel

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            ForEach(references, id: \.title) { reference in
                Button {
                    model.show(reference: reference)
                } label: {
                    Label(reference.title, systemImage: "book")
                }
                .buttonStyle(.link)
                .font(.callout)
            }
        }
    }
}

extension Text {
    /// Render Markdown (bold, italics, lists) from AI replies, falling back to plain text
    init(markdown: String) {
        let options = AttributedString.MarkdownParsingOptions(
            interpretedSyntax: .inlineOnlyPreservingWhitespace
        )
        if let attributed = try? AttributedString(markdown: markdown, options: options) {
            self.init(attributed)
        } else {
            self.init(markdown)
        }
    }
}
