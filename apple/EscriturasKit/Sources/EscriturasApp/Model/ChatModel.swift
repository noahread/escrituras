import EscriturasKit
import Foundation
import Observation

/// Sends questions to an AI provider. Abstracted so the chat can be tested
/// without network access.
public protocol ChatBackend: Sendable {
    /// Stream a reply, passing each piece to `onDelta`, and return the full reply
    func ask(
        provider: AiProvider,
        model: String,
        apiKey: String?,
        history: [Message],
        context: StudyContextInput,
        onDelta: @escaping @Sendable (String) -> Void
    ) async throws -> String

    /// Models available for a provider
    func models(for provider: AiProvider) async -> [String]
}

/// The real backend: escrituras-core through the Library
public struct LibraryChatBackend: ChatBackend {
    let library: Library

    public init(library: Library) {
        self.library = library
    }

    public func ask(
        provider: AiProvider,
        model: String,
        apiKey: String?,
        history: [Message],
        context: StudyContextInput,
        onDelta: @escaping @Sendable (String) -> Void
    ) async throws -> String {
        try await library.ask(
            provider: provider,
            model: model,
            apiKey: apiKey,
            history: history,
            context: context,
            listener: ClosureReplyListener(onDelta)
        )
    }

    public func models(for provider: AiProvider) async -> [String] {
        await library.models(provider: provider)
    }
}

/// Adapts a closure to the ReplyListener callback interface
final class ClosureReplyListener: ReplyListener {
    private let handler: @Sendable (String) -> Void

    init(_ handler: @escaping @Sendable (String) -> Void) {
        self.handler = handler
    }

    func onDelta(text: String) {
        handler(text)
    }
}

public struct ChatEntry: Identifiable, Equatable {
    public enum Role: Equatable {
        case user, assistant, error
    }

    public let id = UUID()
    public let role: Role
    public let text: String
    /// Scripture references found in an assistant reply
    public let references: [ScriptureRef]
}

/// The AI conversation in the inspector
@Observable
@MainActor
public final class ChatModel {
    public private(set) var entries: [ChatEntry] = []
    /// The reply so far while one is streaming in
    public private(set) var streamingReply = ""
    public private(set) var isResponding = false
    public var draft = ""

    private let backend: ChatBackend
    private let library: Library
    private let keys: APIKeyStore
    private var task: Task<Void, Never>?

    public init(backend: ChatBackend, library: Library, keys: APIKeyStore = APIKeyStore()) {
        self.backend = backend
        self.library = library
        self.keys = keys
    }

    /// Ask `question` (or the current draft), with what the user is reading as context
    public func send(
        _ question: String? = nil,
        reading: AppModel,
        settings: AISettings = .current()
    ) {
        let text = (question ?? draft).trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty, !isResponding else { return }
        if question == nil { draft = "" }

        let apiKey = settings.provider.needsAPIKey ? keys.key(for: settings.provider) : nil
        if settings.provider.needsAPIKey && apiKey == nil {
            entries.append(ChatEntry(role: .user, text: text, references: []))
            entries.append(ChatEntry(
                role: .error,
                text: "Add your \(settings.provider.displayName) API key in Settings to ask questions.",
                references: []
            ))
            return
        }

        // Earlier errors aren't part of the conversation sent to the model
        let history = entries.compactMap { entry -> Message? in
            switch entry.role {
            case .user: Message(role: .user, content: entry.text)
            case .assistant: Message(role: .assistant, content: entry.text)
            case .error: nil
            }
        } + [Message(role: .user, content: text)]

        let context = StudyContextInput(
            currentReading: reading.location?.title,
            browsedChapters: reading.browsedChapters,
            includeSavedVerses: settings.includeSavedVerses
        )

        entries.append(ChatEntry(role: .user, text: text, references: []))
        streamingReply = ""
        isResponding = true

        // Pieces arrive on a background thread; the stream keeps them in order
        let (deltas, continuation) = AsyncStream.makeStream(of: String.self)
        let backend = backend
        task = Task {
            let consumer = Task { @MainActor in
                for await delta in deltas {
                    self.streamingReply += delta
                }
            }
            do {
                let reply = try await backend.ask(
                    provider: settings.provider,
                    model: settings.model,
                    apiKey: apiKey,
                    history: history,
                    context: context,
                    onDelta: { continuation.yield($0) }
                )
                continuation.finish()
                await consumer.value
                finish(reply: reply)
            } catch {
                continuation.finish()
                await consumer.value
                finish(error: error)
            }
        }
    }

    /// Ask about a verse, e.g. from its context menu
    public func ask(about verse: Verse, reading: AppModel) {
        send("What does \(verse.title) teach? \"\(verse.text)\"", reading: reading)
    }

    /// Models available for a provider (for Ollama, the ones installed locally)
    public func models(for provider: AiProvider) async -> [String] {
        await backend.models(for: provider)
    }

    public func clear() {
        task?.cancel()
        entries = []
        streamingReply = ""
        isResponding = false
    }

    /// Wait for the current reply (for tests)
    func waitForReply() async {
        await task?.value
    }

    private func finish(reply: String) {
        let references = library.extractReferences(text: reply)
        entries.append(ChatEntry(role: .assistant, text: reply, references: references))
        streamingReply = ""
        isResponding = false
    }

    private func finish(error: Error) {
        // Keep whatever arrived before the failure
        if !streamingReply.isEmpty {
            entries.append(ChatEntry(role: .assistant, text: streamingReply, references: []))
        }
        entries.append(ChatEntry(role: .error, text: Self.describe(error), references: []))
        streamingReply = ""
        isResponding = false
    }

    static func describe(_ error: Error) -> String {
        if let error = error as? EscriturasError {
            switch error {
            case .Data(let message), .Storage(let message), .Ai(let message):
                return message
            }
        }
        return error.localizedDescription
    }
}
