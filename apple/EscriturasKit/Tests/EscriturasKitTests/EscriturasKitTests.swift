import XCTest
@testable import EscriturasKit

final class EscriturasKitTests: XCTestCase {
    /// The repository root, which holds the bundled scripture data
    static let repoRoot = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent() // EscriturasKitTests
        .deletingLastPathComponent() // Tests
        .deletingLastPathComponent() // EscriturasKit
        .deletingLastPathComponent() // apple
        .deletingLastPathComponent() // repository root

    func makeLibrary(studyDatabase: URL? = nil) throws -> Library {
        try Library.open(dataRoot: Self.repoRoot, studyDatabase: studyDatabase)
    }

    func testBrowsing() throws {
        let library = try makeLibrary()
        XCTAssertEqual(library.volumes().count, 5)
        XCTAssertEqual(library.chapters(book: "1 Nephi").count, 22)

        let verses = library.verses(book: "Moroni", chapter: 10)
        XCTAssertEqual(verses.count, 34)
        XCTAssertEqual(verses[3].title, "Moroni 10:4")

        XCTAssertEqual(
            library.nextChapter(book: "1 Nephi", chapter: 22),
            ChapterRef(book: "2 Nephi", chapter: 1)
        )
    }

    func testLookupSearchAndReferences() throws {
        let library = try makeLibrary()
        XCTAssertEqual(library.lookup(reference: "Mosiah 4:19-21").map(\.number), [19, 20, 21])
        XCTAssertTrue(
            library.search(query: "charity never faileth", limit: 5)
                .contains { $0.verse.title == "1 Corinthians 13:8" }
        )
        XCTAssertEqual(
            library.extractReferences(text: "Alma 32:21, Ether 12:6.").map(\.title),
            ["Alma 32:21", "Ether 12:6"]
        )
    }

    func testMissingDataThrows() {
        XCTAssertThrowsError(
            try Library.open(dataRoot: URL(fileURLWithPath: "/nonexistent"), studyDatabase: nil)
        ) { error in
            guard case EscriturasError.Data = error else {
                return XCTFail("Expected a data error, got \(error)")
            }
        }
    }

    func testStudyDataPersists() throws {
        let database = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString)
            .appendingPathComponent("study.db")

        do {
            let library = try makeLibrary(studyDatabase: database)
            XCTAssertTrue(try library.saveVerse(title: "Alma 32:21"))
            try library.setNote(verseTitle: "Alma 32:21", body: "Faith is hope")
            try library.recordReading(book: "Alma", chapter: 32)
        }

        let library = try makeLibrary(studyDatabase: database)
        XCTAssertEqual(try library.savedVerses().map(\.title), ["Alma 32:21"])
        XCTAssertEqual(try library.note(verseTitle: "Alma 32:21")?.body, "Faith is hope")
        XCTAssertEqual(try library.lastPosition(), ChapterRef(book: "Alma", chapter: 32))
    }

    final class Collector: ReplyListener, @unchecked Sendable {
        var deltas: [String] = []
        func onDelta(text: String) { deltas.append(text) }
    }

    func testAskWithoutApiKeyThrows() async throws {
        let library = try makeLibrary()
        let collector = Collector()
        do {
            _ = try await library.ask(
                provider: .claude,
                model: "claude-opus-5-5",
                apiKey: nil,
                history: [Message(role: .user, content: "What is faith?")],
                context: StudyContextInput(
                    currentReading: "Alma 32", browsedChapters: [], includeSavedVerses: false
                ),
                listener: collector
            )
            XCTFail("Expected an error")
        } catch EscriturasError.Ai {
            XCTAssertTrue(collector.deltas.isEmpty)
        }
    }
}
