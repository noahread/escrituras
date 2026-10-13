import EscriturasKit
import XCTest
@testable import EscriturasApp

/// Streams a canned reply in pieces, or fails partway
struct FakeChatBackend: ChatBackend {
    var pieces: [String] = []
    var failure: String?
    let received = Received()

    final class Received: @unchecked Sendable {
        var history: [Message] = []
        var context: StudyContextInput?
        var apiKey: String?
    }

    func ask(
        provider: AiProvider,
        model: String,
        apiKey: String?,
        history: [Message],
        context: StudyContextInput,
        onDelta: @escaping @Sendable (String) -> Void
    ) async throws -> String {
        received.history = history
        received.context = context
        received.apiKey = apiKey
        for piece in pieces {
            onDelta(piece)
            await Task.yield()
        }
        if let failure {
            throw EscriturasError.Ai(message: failure)
        }
        return pieces.joined()
    }

    func models(for provider: AiProvider) async -> [String] { ["mock"] }
}

@MainActor
final class ChatModelTests: XCTestCase {
    let ollama = AISettings(provider: .ollama, model: "mock", includeSavedVerses: true)

    func testStreamsReplyAndExtractsReferences() async throws {
        let library = try makeLibrary()
        let reading = AppModel(library: library)
        reading.show(ChapterRef(book: "Alma", chapter: 32))

        let backend = FakeChatBackend(pieces: ["Faith is hope ", "(Alma 32:21; ", "Ether 12:6)."])
        let chat = ChatModel(backend: backend, library: library, keys: APIKeyStore(service: "test-\(UUID())"))

        chat.send("What is faith?", reading: reading, settings: ollama)
        XCTAssertTrue(chat.isResponding)
        await chat.waitForReply()

        XCTAssertFalse(chat.isResponding)
        XCTAssertEqual(chat.streamingReply, "")
        XCTAssertEqual(chat.entries.map(\.role), [.user, .assistant])
        XCTAssertEqual(chat.entries.last?.text, "Faith is hope (Alma 32:21; Ether 12:6).")
        XCTAssertEqual(chat.entries.last?.references.map(\.title), ["Alma 32:21", "Ether 12:6"])

        // What the user is reading goes along as context
        XCTAssertEqual(backend.received.context?.currentReading, "Alma 32")
        XCTAssertEqual(backend.received.context?.browsedChapters, [ChapterRef(book: "Alma", chapter: 32)])
        XCTAssertEqual(backend.received.context?.includeSavedVerses, true)
        XCTAssertNil(backend.received.apiKey)
    }

    func testFollowUpSendsConversationWithoutErrors() async throws {
        let library = try makeLibrary()
        let reading = AppModel(library: library)
        let keys = APIKeyStore(service: "test-\(UUID())")

        let failing = FakeChatBackend(pieces: ["Partial "], failure: "Connection lost")
        let chat = ChatModel(backend: failing, library: library, keys: keys)
        chat.send("First question", reading: reading, settings: ollama)
        await chat.waitForReply()
        // The partial reply is kept, followed by the error
        XCTAssertEqual(chat.entries.map(\.role), [.user, .assistant, .error])
        XCTAssertEqual(chat.entries[1].text, "Partial ")
        XCTAssertEqual(chat.entries[2].text, "Connection lost")

        let backend = FakeChatBackend(pieces: ["Answer"])
        let followUp = ChatModel(backend: backend, library: library, keys: keys)
        followUp.send("One", reading: reading, settings: ollama)
        await followUp.waitForReply()
        followUp.send("Two", reading: reading, settings: ollama)
        await followUp.waitForReply()
        XCTAssertEqual(backend.received.history.map(\.content), ["One", "Answer", "Two"])
        XCTAssertEqual(backend.received.history.map(\.role), [.user, .assistant, .user])
    }

    func testMissingApiKeyExplainsWhatToDo() async throws {
        let library = try makeLibrary()
        let backend = FakeChatBackend(pieces: ["unused"])
        let chat = ChatModel(backend: backend, library: library, keys: APIKeyStore(service: "test-\(UUID())"))

        chat.send(
            "What is faith?",
            reading: AppModel(library: library),
            settings: AISettings(provider: .claude, model: "claude-opus-5-5", includeSavedVerses: true)
        )
        XCTAssertFalse(chat.isResponding)
        XCTAssertEqual(chat.entries.map(\.role), [.user, .error])
        XCTAssertTrue(chat.entries[1].text.contains("API key in Settings"))
        XCTAssertTrue(backend.received.history.isEmpty)
    }

    func testDraftIsClearedWhenSent() async throws {
        let library = try makeLibrary()
        let chat = ChatModel(backend: FakeChatBackend(pieces: ["ok"]), library: library, keys: APIKeyStore(service: "test-\(UUID())"))
        chat.draft = "  What is charity?  "
        chat.send(reading: AppModel(library: library), settings: ollama)
        XCTAssertEqual(chat.draft, "")
        await chat.waitForReply()
        XCTAssertEqual(chat.entries.first?.text, "What is charity?")
    }
}

final class SettingsTests: XCTestCase {
    func testProviderNamesRoundTrip() {
        for provider in AiProvider.allCases {
            XCTAssertEqual(AiProvider(storageName: provider.storageName), provider)
        }
        XCTAssertNil(AiProvider(storageName: "unknown"))
    }

    func testDefaultsWhenNothingIsSet() throws {
        let defaults = try XCTUnwrap(UserDefaults(suiteName: "test-\(UUID())"))
        let settings = AISettings.current(defaults: defaults)
        XCTAssertEqual(settings.provider, .claude)
        XCTAssertEqual(settings.model, "claude-opus-5-5")
        XCTAssertTrue(settings.includeSavedVerses)

        defaults.set("ollama", forKey: SettingsKey.provider)
        defaults.set("llama3", forKey: SettingsKey.model(for: .ollama))
        defaults.set(false, forKey: SettingsKey.includeSavedVerses)
        let changed = AISettings.current(defaults: defaults)
        XCTAssertEqual(changed.provider, .ollama)
        XCTAssertEqual(changed.model, "llama3")
        XCTAssertFalse(changed.includeSavedVerses)
    }
}
