import EscriturasKit
import Foundation

/// User defaults keys, shared by @AppStorage in views and the models
public enum SettingsKey {
    public static let provider = "AIProvider"
    public static let includeSavedVerses = "IncludeSavedVersesInQuestions"
    public static let readerFontSize = "ReaderFontSize"

    /// The selected model for a provider
    public static func model(for provider: AiProvider) -> String {
        "AIModel.\(provider.storageName)"
    }
}

public enum ReaderFont {
    public static let defaultSize: Double = 16
    public static let range: ClosedRange<Double> = 11...32
}

extension AiProvider: CaseIterable, Identifiable {
    public static var allCases: [AiProvider] { [.claude, .openAi, .ollama] }

    public var id: String { storageName }

    /// Stable name for settings and the Keychain
    public var storageName: String {
        switch self {
        case .claude: "claude"
        case .openAi: "openai"
        case .ollama: "ollama"
        }
    }

    public init?(storageName: String) {
        guard let match = Self.allCases.first(where: { $0.storageName == storageName }) else {
            return nil
        }
        self = match
    }

    public var displayName: String {
        switch self {
        case .claude: "Claude (Anthropic)"
        case .openAi: "ChatGPT (OpenAI)"
        case .ollama: "Ollama (on this Mac)"
        }
    }

    public var needsAPIKey: Bool { self != .ollama }

    /// Used until the user picks a model
    public var defaultModel: String {
        switch self {
        case .claude: "claude-opus-5-5"
        case .openAi: "gpt-4o"
        case .ollama: "gemma3:latest"
        }
    }
}

/// Reads the AI settings from user defaults
public struct AISettings {
    public var provider: AiProvider
    public var model: String
    public var includeSavedVerses: Bool

    public static func current(defaults: UserDefaults = .standard) -> AISettings {
        let provider = defaults.string(forKey: SettingsKey.provider)
            .flatMap(AiProvider.init(storageName:)) ?? .claude
        let model = defaults.string(forKey: SettingsKey.model(for: provider))
            .flatMap { $0.isEmpty ? nil : $0 } ?? provider.defaultModel
        let includeSaved = defaults.object(forKey: SettingsKey.includeSavedVerses) as? Bool ?? true
        return AISettings(provider: provider, model: model, includeSavedVerses: includeSaved)
    }
}
