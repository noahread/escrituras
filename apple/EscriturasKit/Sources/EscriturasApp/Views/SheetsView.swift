import EscriturasKit
import SwiftUI

/// Add or edit the note on a verse
struct NoteEditorView: View {
    let model: AppModel
    let verse: Verse
    @State private var text = ""
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Note on \(verse.title)")
                .font(.headline)
            Text(verse.text)
                .font(.callout)
                .foregroundStyle(.secondary)
                .lineLimit(4)
            TextEditor(text: $text)
                .font(.body)
                .frame(minHeight: 140)
                .scrollContentBackground(.hidden)
                .padding(6)
                .background(.background, in: RoundedRectangle(cornerRadius: 6))
                .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(.separator))
            HStack {
                if model.note(for: verse) != nil {
                    Button("Delete Note", role: .destructive) {
                        model.setNote("", for: verse)
                        dismiss()
                    }
                }
                Spacer()
                Button("Cancel", role: .cancel) { dismiss() }
                    .keyboardShortcut(.cancelAction)
                Button("Save") {
                    model.setNote(text, for: verse)
                    dismiss()
                }
                .keyboardShortcut(.defaultAction)
            }
        }
        .padding(20)
        .frame(width: 460)
        .onAppear { text = model.note(for: verse) ?? "" }
    }
}

/// Jump to a reference such as "Alma 32:21" (⌘L)
struct GoToReferenceView: View {
    let model: AppModel
    @State private var reference = ""
    @State private var notFound = false
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Go to Reference")
                .font(.headline)
            TextField("e.g. Alma 32:21, Mosiah 4:19-21 or Moroni 10", text: $reference)
                .textFieldStyle(.roundedBorder)
                .onSubmit(go)
                .onChange(of: reference) { notFound = false }
            if notFound {
                Text("“\(reference)” isn't a reference I recognize.")
                    .font(.callout)
                    .foregroundStyle(.red)
            }
            HStack {
                Spacer()
                Button("Cancel", role: .cancel) { dismiss() }
                    .keyboardShortcut(.cancelAction)
                Button("Go", action: go)
                    .keyboardShortcut(.defaultAction)
                    .disabled(reference.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
        .padding(20)
        .frame(width: 380)
    }

    private func go() {
        if model.goTo(reference: reference) {
            dismiss()
        } else {
            notFound = true
        }
    }
}
