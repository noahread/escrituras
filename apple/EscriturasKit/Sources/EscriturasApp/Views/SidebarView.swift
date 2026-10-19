import EscriturasKit
import SwiftUI

/// Volumes as sections, with their books
struct SidebarView: View {
    @Bindable var model: AppModel

    var body: some View {
        ScrollViewReader { proxy in
            List(selection: $model.selectedBook) {
                ForEach(model.volumes, id: \.self) { volume in
                    Section(volume) {
                        ForEach(model.books(in: volume), id: \.self) { book in
                            Label(book, systemImage: "book.closed")
                                .tag(book)
                                .id(book)
                        }
                    }
                }
            }
            .listStyle(.sidebar)
            // Keep the selected book visible, e.g. after jumping to a reference
            .task(id: model.selectedBook) {
                if let book = model.selectedBook {
                    proxy.scrollTo(book, anchor: .center)
                }
            }
        }
    }
}

/// The chapters of the book selected in the sidebar
struct ChapterListView: View {
    @Bindable var model: AppModel

    var body: some View {
        if let book = model.selectedBook {
            ScrollViewReader { proxy in
                List(selection: chapterSelection(book: book)) {
                    ForEach(model.chapters(of: book), id: \.self) { chapter in
                        Text(model.chapterLabel(book: book, chapter: chapter))
                            .tag(chapter)
                            .id(chapter)
                    }
                }
                // Keep the open chapter visible, e.g. after jumping to a reference
                .task(id: model.location) {
                    if let location = model.location, location.book == book {
                        proxy.scrollTo(location.chapter, anchor: .center)
                    }
                }
            }
        } else {
            ContentUnavailableView("Choose a Book", systemImage: "books.vertical")
        }
    }

    /// Selecting a chapter opens it in the reader
    private func chapterSelection(book: String) -> Binding<Int32?> {
        Binding(
            get: { model.location?.book == book ? model.location?.chapter : nil },
            set: { chapter in
                if let chapter {
                    model.show(ChapterRef(book: book, chapter: chapter))
                }
            }
        )
    }
}

/// Search results replace the chapter list while searching
struct SearchResultsView: View {
    @Bindable var model: AppModel

    var body: some View {
        Group {
            if model.isSearching && model.searchResults.isEmpty {
                ProgressView("Searching…")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if model.searchResults.isEmpty {
                ContentUnavailableView.search(text: model.searchText)
            } else {
                List(selection: resultSelection) {
                    ForEach(model.searchResults, id: \.verse.title) { result in
                        VStack(alignment: .leading, spacing: 2) {
                            Text(result.verse.title)
                                .font(.headline)
                            Text(result.verse.text)
                                .font(.callout)
                                .foregroundStyle(.secondary)
                                .lineLimit(3)
                        }
                        .padding(.vertical, 2)
                        .tag(result.verse.title)
                    }
                }
            }
        }
    }

    private var resultSelection: Binding<String?> {
        Binding(
            get: { model.selectedVerse },
            set: { title in
                if let result = model.searchResults.first(where: { $0.verse.title == title }) {
                    model.show(verse: result.verse)
                }
            }
        )
    }
}
