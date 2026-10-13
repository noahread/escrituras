import EscriturasKit
import Foundation
import Observation

extension Verse: Identifiable {
    public var id: String { title }
}

extension ChapterRef {
    /// e.g. "Alma 32"
    public var title: String { "\(book) \(chapter)" }
}

/// A verse with the user's note on it
public struct NotedVerse: Identifiable {
    public let verse: Verse
    public let note: VerseNote
    public var id: String { verse.title }
}

/// What the main window shows and everything the user can do to it.
@Observable
@MainActor
public final class AppModel {
    public let library: Library
    public let volumes: [String]

    // MARK: Reading

    /// The chapter in the reader
    public private(set) var location: ChapterRef?
    public private(set) var verses: [Verse] = []
    /// The verse with keyboard focus / context-menu target
    public var selectedVerse: String?
    /// Verses highlighted after jumping to a reference or search result
    public private(set) var highlightedVerses: ClosedRange<Int32>?
    /// Set when the reader should scroll to a verse
    public private(set) var scrollTarget: String?
    /// The book whose chapters the middle column lists
    public var selectedBook: String?

    private var backStack: [ChapterRef] = []
    private var forwardStack: [ChapterRef] = []
    /// Chapters opened this session, oldest first (sent as AI context)
    public private(set) var browsedChapters: [ChapterRef] = []

    // MARK: Search

    public var searchText = ""
    public private(set) var searchResults: [SearchResult] = []
    public private(set) var isSearching = false
    @ObservationIgnored private var searchTask: Task<Void, Never>?

    // MARK: Study data

    public private(set) var savedVerses: [Verse] = []
    public private(set) var notes: [String: VerseNote] = [:]

    // MARK: Window state

    public var showInspector = false
    public var inspectorTab: InspectorTab = .study
    public var showGoToReference = false
    /// The verse whose note is being edited
    public var noteEditorVerse: Verse?
    public var errorMessage: String?

    public enum InspectorTab: String, CaseIterable, Identifiable {
        case study, ask
        public var id: String { rawValue }
    }

    // Caches filled while views render, so they must not be observed
    @ObservationIgnored private var booksByVolume: [String: [String]] = [:]
    @ObservationIgnored private var chaptersByBook: [String: [Int32]] = [:]

    public init(library: Library) {
        self.library = library
        volumes = library.volumes()
        reloadStudyData()
    }

    // MARK: - Browsing

    public func books(in volume: String) -> [String] {
        if let cached = booksByVolume[volume] { return cached }
        let books = library.books(volume: volume)
        booksByVolume[volume] = books
        return books
    }

    public func chapters(of book: String) -> [Int32] {
        if let cached = chaptersByBook[book] { return cached }
        let chapters = library.chapters(book: book)
        chaptersByBook[book] = chapters
        return chapters
    }

    /// "Section 76" for the Doctrine and Covenants, "Chapter 32" otherwise
    public func chapterLabel(book: String, chapter: Int32) -> String {
        book == "Doctrine and Covenants" ? "Section \(chapter)" : "Chapter \(chapter)"
    }

    /// Window title, e.g. "Alma 32"
    public var title: String { location?.title ?? "Escrituras" }

    public var volumeOfLocation: String? {
        location.flatMap { library.volumeForBook(book: $0.book) }
    }

    /// Open the chapter the user last read, or the first chapter of the Book of Mormon
    public func openStartingChapter() {
        if let last = try? library.lastPosition(), show(last, recordHistory: false) {
            return
        }
        let book = books(in: "Book of Mormon").first ?? volumes.first.flatMap { books(in: $0).first }
        if let book, let first = chapters(of: book).first {
            show(ChapterRef(book: book, chapter: first), recordHistory: false)
        }
    }

    /// Show a chapter. Returns false if it doesn't exist.
    @discardableResult
    public func show(
        _ chapter: ChapterRef,
        highlighting range: ClosedRange<Int32>? = nil,
        recordHistory: Bool = true
    ) -> Bool {
        let newVerses = library.verses(book: chapter.book, chapter: chapter.chapter)
        guard !newVerses.isEmpty else { return false }

        if recordHistory, let current = location, current != chapter {
            backStack.append(current)
            forwardStack.removeAll()
        }
        location = chapter
        verses = newVerses
        selectedBook = chapter.book
        highlightedVerses = range
        let target = range.flatMap { range in newVerses.first { $0.number == range.lowerBound } }
        selectedVerse = target?.title ?? newVerses.first?.title
        scrollTarget = target?.title ?? newVerses.first?.title

        if !browsedChapters.contains(chapter) {
            browsedChapters.append(chapter)
        }
        try? library.recordReading(book: chapter.book, chapter: chapter.chapter)
        return true
    }

    /// Open the chapter containing `verse` and highlight it
    public func show(verse: Verse) {
        show(ChapterRef(book: verse.book, chapter: verse.chapter), highlighting: verse.number...verse.number)
    }

    /// Open a reference such as "Alma 32:21", "Mosiah 4:19-21" or "Alma 32".
    /// Returns false if it isn't recognized.
    @discardableResult
    public func goTo(reference: String) -> Bool {
        let trimmed = reference.trimmingCharacters(in: .whitespacesAndNewlines)
        let matches = library.lookup(reference: trimmed)
        if let first = matches.first, let last = matches.last {
            return show(ChapterRef(book: first.book, chapter: first.chapter), highlighting: first.number...last.number)
        }
        // A whole chapter, e.g. "Alma 32" or "Doctrine and Covenants 76"
        if let space = trimmed.lastIndex(of: " "),
           let chapter = Int32(trimmed[trimmed.index(after: space)...]) {
            let name = String(trimmed[..<space])
            if let book = allBooks.first(where: { $0.caseInsensitiveCompare(name) == .orderedSame }) {
                return show(ChapterRef(book: book, chapter: chapter))
            }
        }
        return false
    }

    /// Open a reference found in an AI reply
    public func show(reference: ScriptureRef) {
        show(ChapterRef(book: reference.book, chapter: reference.chapter),
             highlighting: reference.startVerse...max(reference.startVerse, reference.endVerse))
    }

    private var allBooks: [String] { volumes.flatMap { books(in: $0) } }

    // MARK: - Chapter and history navigation

    public var canGoBack: Bool { !backStack.isEmpty }
    public var canGoForward: Bool { !forwardStack.isEmpty }

    public func goBack() {
        guard let previous = backStack.popLast() else { return }
        if let current = location { forwardStack.append(current) }
        show(previous, recordHistory: false)
    }

    public func goForward() {
        guard let next = forwardStack.popLast() else { return }
        if let current = location { backStack.append(current) }
        show(next, recordHistory: false)
    }

    public var nextChapter: ChapterRef? {
        location.flatMap { library.nextChapter(book: $0.book, chapter: $0.chapter) }
    }

    public var previousChapter: ChapterRef? {
        location.flatMap { library.previousChapter(book: $0.book, chapter: $0.chapter) }
    }

    public func goToNextChapter() {
        if let next = nextChapter { show(next) }
    }

    public func goToPreviousChapter() {
        if let previous = previousChapter { show(previous) }
    }

    /// Move the verse selection by `offset` within the chapter
    public func moveSelection(by offset: Int) {
        guard !verses.isEmpty else { return }
        let current = verses.firstIndex { $0.title == selectedVerse } ?? 0
        let index = min(max(current + offset, 0), verses.count - 1)
        selectedVerse = verses[index].title
        scrollTarget = verses[index].title
    }

    public var selectedVerseValue: Verse? {
        verses.first { $0.title == selectedVerse }
    }

    // MARK: - Search

    /// Search for `searchText` after a short pause, so typing stays responsive
    public func scheduleSearch(delay: Duration = .milliseconds(250)) {
        searchTask?.cancel()
        let query = searchText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else {
            searchResults = []
            isSearching = false
            return
        }
        isSearching = true
        let library = library
        searchTask = Task {
            try? await Task.sleep(for: delay)
            guard !Task.isCancelled else { return }
            // Semantic search can take a moment (and loads its model on
            // first use), so run it off the main thread
            let results = await Task.detached(priority: .userInitiated) {
                library.search(query: query, limit: 50)
            }.value
            guard !Task.isCancelled else { return }
            searchResults = results
            isSearching = false
        }
    }

    /// Wait for the search scheduled by `scheduleSearch` (for tests)
    func waitForSearch() async {
        await searchTask?.value
    }

    // MARK: - Saved verses and notes

    public func isSaved(_ verse: Verse) -> Bool {
        savedVerses.contains { $0.title == verse.title }
    }

    public func toggleSaved(_ verse: Verse) {
        do {
            if isSaved(verse) {
                try library.removeSavedVerse(title: verse.title)
            } else {
                _ = try library.saveVerse(title: verse.title)
            }
        } catch {
            errorMessage = error.localizedDescription
        }
        reloadStudyData()
    }

    public func note(for verse: Verse) -> String? {
        notes[verse.title]?.body
    }

    /// Set a verse's note; blank text deletes it
    public func setNote(_ body: String, for verse: Verse) {
        do {
            try library.setNote(verseTitle: verse.title, body: body)
        } catch {
            errorMessage = error.localizedDescription
        }
        reloadStudyData()
    }

    /// Notes with their verses, most recently updated first
    public var notedVerses: [NotedVerse] {
        notes.values
            .sorted { $0.updatedAt > $1.updatedAt }
            .compactMap { note in library.verse(title: note.verseTitle).map { NotedVerse(verse: $0, note: note) } }
    }

    public func reloadStudyData() {
        do {
            let saved = try library.savedVerses()
            let allNotes = try library.notes()
            savedVerses = saved
            notes = Dictionary(uniqueKeysWithValues: allNotes.map { ($0.verseTitle, $0) })
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    // MARK: - Copying

    /// "Alma 32:21\nAnd now as I said concerning faith..."
    public static func clipboardText(for verse: Verse) -> String {
        "\(verse.title)\n\(verse.text)"
    }
}
