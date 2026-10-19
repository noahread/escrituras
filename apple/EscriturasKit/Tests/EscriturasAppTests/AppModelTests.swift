import EscriturasKit
import XCTest
@testable import EscriturasApp

/// The repository root, which holds the bundled scripture data
let repoRoot = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent() // EscriturasAppTests
    .deletingLastPathComponent() // Tests
    .deletingLastPathComponent() // EscriturasKit
    .deletingLastPathComponent() // apple
    .deletingLastPathComponent() // repository root

/// A library on the bundled data with an empty study database
func makeLibrary() throws -> Library {
    let database = FileManager.default.temporaryDirectory
        .appendingPathComponent(UUID().uuidString)
        .appendingPathComponent("study.db")
    return try Library.open(dataRoots: [repoRoot.path], studyDbPath: database.path)
}

@MainActor
final class AppModelTests: XCTestCase {
    func makeModel() throws -> AppModel {
        AppModel(library: try makeLibrary())
    }

    func testStartsAtBookOfMormonWithoutHistory() throws {
        let model = try makeModel()
        model.openStartingChapter()
        XCTAssertEqual(model.location, ChapterRef(book: "1 Nephi", chapter: 1))
        XCTAssertEqual(model.selectedBook, "1 Nephi")
        XCTAssertEqual(model.verses.first?.title, "1 Nephi 1:1")
        XCTAssertEqual(model.title, "1 Nephi 1")
        XCTAssertEqual(model.volumeOfLocation, "Book of Mormon")
    }

    func testResumesLastChapterRead() throws {
        let library = try makeLibrary()
        AppModel(library: library).show(ChapterRef(book: "Alma", chapter: 32))

        let reopened = AppModel(library: library)
        reopened.openStartingChapter()
        XCTAssertEqual(reopened.location, ChapterRef(book: "Alma", chapter: 32))
    }

    func testChapterNavigationCrossesBooks() throws {
        let model = try makeModel()
        model.show(ChapterRef(book: "1 Nephi", chapter: 22))
        XCTAssertEqual(model.nextChapter, ChapterRef(book: "2 Nephi", chapter: 1))

        model.goToNextChapter()
        XCTAssertEqual(model.location, ChapterRef(book: "2 Nephi", chapter: 1))
        XCTAssertEqual(model.selectedBook, "2 Nephi")

        model.goToPreviousChapter()
        XCTAssertEqual(model.location, ChapterRef(book: "1 Nephi", chapter: 22))
    }

    func testBackAndForward() throws {
        let model = try makeModel()
        model.show(ChapterRef(book: "Alma", chapter: 32))
        model.show(ChapterRef(book: "Ether", chapter: 12))
        model.show(ChapterRef(book: "Moroni", chapter: 10))
        XCTAssertTrue(model.canGoBack)
        XCTAssertFalse(model.canGoForward)

        model.goBack()
        XCTAssertEqual(model.location?.title, "Ether 12")
        model.goBack()
        XCTAssertEqual(model.location?.title, "Alma 32")
        XCTAssertFalse(model.canGoBack)

        model.goForward()
        XCTAssertEqual(model.location?.title, "Ether 12")

        // Opening a new chapter clears the forward history
        model.show(ChapterRef(book: "John", chapter: 3))
        XCTAssertFalse(model.canGoForward)
    }

    func testGoToReferenceHighlightsVerses() throws {
        let model = try makeModel()
        XCTAssertTrue(model.goTo(reference: "Mosiah 4:19-21"))
        XCTAssertEqual(model.location, ChapterRef(book: "Mosiah", chapter: 4))
        XCTAssertEqual(model.highlightedVerses, 19...21)
        XCTAssertEqual(model.selectedVerse, "Mosiah 4:19")
        XCTAssertEqual(model.scrollTarget, "Mosiah 4:19")

        XCTAssertTrue(model.goTo(reference: "moroni 10"))
        XCTAssertEqual(model.location, ChapterRef(book: "Moroni", chapter: 10))
        XCTAssertNil(model.highlightedVerses)

        XCTAssertFalse(model.goTo(reference: "Nowhere 1:1"))
        XCTAssertEqual(model.location, ChapterRef(book: "Moroni", chapter: 10))
    }

    func testMoveSelectionStaysInChapter() throws {
        let model = try makeModel()
        model.show(ChapterRef(book: "Moroni", chapter: 10))
        XCTAssertEqual(model.selectedVerse, "Moroni 10:1")
        model.moveSelection(by: 3)
        XCTAssertEqual(model.selectedVerse, "Moroni 10:4")
        model.moveSelection(by: -10)
        XCTAssertEqual(model.selectedVerse, "Moroni 10:1")
        model.moveSelection(by: 100)
        XCTAssertEqual(model.selectedVerse, "Moroni 10:34")
    }

    func testSearch() async throws {
        let model = try makeModel()
        model.searchText = "charity never faileth"
        model.scheduleSearch(delay: .zero)
        XCTAssertTrue(model.isSearching)
        await model.waitForSearch()
        XCTAssertFalse(model.isSearching)
        XCTAssertTrue(model.searchResults.contains { $0.verse.title == "1 Corinthians 13:8" })

        model.searchText = "   "
        model.scheduleSearch()
        XCTAssertTrue(model.searchResults.isEmpty)
        XCTAssertFalse(model.isSearching)
    }

    func testSavedVersesAndNotes() throws {
        let model = try makeModel()
        model.show(ChapterRef(book: "Alma", chapter: 32))
        let verse = try XCTUnwrap(model.verses.first { $0.number == 21 })

        model.toggleSaved(verse)
        XCTAssertTrue(model.isSaved(verse))
        XCTAssertEqual(model.savedVerses.map(\.title), ["Alma 32:21"])
        model.toggleSaved(verse)
        XCTAssertFalse(model.isSaved(verse))

        model.setNote("Faith is hope for things not seen", for: verse)
        XCTAssertEqual(model.note(for: verse), "Faith is hope for things not seen")
        XCTAssertEqual(model.notedVerses.map(\.verse.title), ["Alma 32:21"])
        model.setNote("  ", for: verse)
        XCTAssertNil(model.note(for: verse))
    }

    /// Saved verses and notes share one list in the study panel, so their row
    /// ids must differ even for the same verse, or the rows get mixed up
    func testSavedAndNotedRowsHaveDistinctIDs() throws {
        let model = try makeModel()
        model.show(ChapterRef(book: "Alma", chapter: 32))
        let verse = try XCTUnwrap(model.verses.first { $0.number == 21 })
        model.toggleSaved(verse)
        model.setNote("Faith is hope for things not seen", for: verse)

        let savedIDs = Set(model.savedVerses.map(\.id))
        let notedIDs = Set(model.notedVerses.map(\.id))
        XCTAssertTrue(savedIDs.isDisjoint(with: notedIDs))
        XCTAssertEqual(model.notedVerses.first?.note.body, "Faith is hope for things not seen")
    }

    func testChapterLabelsAndClipboardText() throws {
        let model = try makeModel()
        XCTAssertEqual(model.chapterLabel(book: "Doctrine and Covenants", chapter: 76), "Section 76")
        XCTAssertEqual(model.chapterLabel(book: "Alma", chapter: 32), "Chapter 32")

        let verse = try XCTUnwrap(model.library.verse(title: "John 3:16"))
        XCTAssertTrue(AppModel.clipboardText(for: verse).hasPrefix("John 3:16\nFor God so loved"))
    }

    func testBrowsedChaptersAreRecordedOnce() throws {
        let model = try makeModel()
        model.show(ChapterRef(book: "Alma", chapter: 32))
        model.show(ChapterRef(book: "Ether", chapter: 12))
        model.show(ChapterRef(book: "Alma", chapter: 32))
        XCTAssertEqual(model.browsedChapters.map(\.title), ["Alma 32", "Ether 12"])
    }
}

final class DataLocationTests: XCTestCase {
    func testFindsRepositoryAboveNestedDirectory() {
        let nested = repoRoot.appendingPathComponent("apple/EscriturasKit/Sources")
        XCTAssertEqual(
            DataLocation.repositoryRoot(above: nested)?.standardizedFileURL.path,
            repoRoot.standardizedFileURL.path
        )
        XCTAssertNil(DataLocation.repositoryRoot(above: URL(fileURLWithPath: "/")))
    }

    func testStudyDatabaseIsSharedWithTerminalApp() {
        XCTAssertTrue(DataLocation.studyDatabase().path.hasSuffix("Application Support/escrituras/study.db"))
    }
}
