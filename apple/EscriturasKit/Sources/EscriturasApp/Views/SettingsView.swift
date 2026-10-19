import EscriturasKit
import SwiftUI

/// The Settings window (⌘,)
struct SettingsView: View {
    let chat: ChatModel?

    var body: some View {
        TabView {
            ReadingSettingsView()
                .tabItem { Label("Reading", systemImage: "textformat.size") }
            AISettingsView(chat: chat)
                .tabItem { Label("AI", systemImage: "sparkles") }
        }
        .frame(width: 520)
    }
}

struct ReadingSettingsView: View {
    @AppStorage(SettingsKey.readerFontSize) private var fontSize = ReaderFont.defaultSize

    var body: some View {
        Form {
            Slider(value: $fontSize, in: ReaderFont.range, step: 1) {
                Text("Text size")
            } minimumValueLabel: {
                Image(systemName: "textformat.size.smaller")
            } maximumValueLabel: {
                Image(systemName: "textformat.size.larger")
            }
            Text("And now as I said concerning faith—faith is not to have a perfect knowledge of things.")
                .font(.system(size: fontSize, design: .serif))
                .foregroundStyle(.secondary)
                .lineLimit(3)
                .fixedSize(horizontal: false, vertical: true)
            LabeledContent("Study data") {
                Text(DataLocation.studyDatabase().path)
                    .textSelection(.enabled)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .formStyle(.grouped)
    }
}

struct AISettingsView: View {
    let chat: ChatModel?
    @AppStorage(SettingsKey.provider) private var providerName = AiProvider.claude.storageName
    @AppStorage(SettingsKey.includeSavedVerses) private var includeSavedVerses = true
    @State private var model = ""
    @State private var apiKey = ""
    @State private var keyStatus: String?
    @State private var ollamaModels: [String] = []

    private let keys = APIKeyStore()

    private var provider: AiProvider {
        AiProvider(storageName: providerName) ?? .claude
    }

    var body: some View {
        Form {
            Picker("Provider", selection: $providerName) {
                ForEach(AiProvider.allCases) { provider in
                    Text(provider.displayName).tag(provider.storageName)
                }
            }

            modelPicker

            if provider.needsAPIKey {
                LabeledContent("API key") {
                    HStack {
                        SecureField("Paste your key", text: $apiKey)
                            .labelsHidden()
                        Button("Save") { saveKey() }
                            .disabled(apiKey.isEmpty)
                    }
                }
                Text(keyStatus ?? (keys.key(for: provider) == nil
                    ? "No key saved yet. Keys are stored in your Keychain."
                    : "A key is saved in your Keychain."))
                    .font(.caption)
                    .foregroundStyle(.secondary)
            } else {
                Text("Ollama runs models on this Mac. Install it from ollama.com and run `ollama serve`.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Toggle("Include saved verses when asking questions", isOn: $includeSavedVerses)
        }
        .formStyle(.grouped)
        .onAppear(perform: loadModel)
        .onChange(of: providerName) {
            apiKey = ""
            keyStatus = nil
            loadModel()
        }
        .onChange(of: model) {
            UserDefaults.standard.set(model, forKey: SettingsKey.model(for: provider))
        }
    }

    @ViewBuilder
    private var modelPicker: some View {
        switch provider {
        case .claude:
            Picker("Model", selection: $model) {
                Text("Claude Opus 5.5").tag("claude-opus-5-5")
                Text("Claude Sonnet 5.5").tag("claude-sonnet-5-5")
                Text("Claude Haiku 4.5").tag("claude-haiku-4-5")
                Text("Claude Fable 5.1").tag("claude-fable-5-1")
                if !Self.claudeModels.contains(model) && !model.isEmpty {
                    Text(model).tag(model)
                }
            }
        case .ollama where !ollamaModels.isEmpty:
            Picker("Model", selection: $model) {
                ForEach(ollamaModels, id: \.self) { Text($0).tag($0) }
                if !ollamaModels.contains(model) && !model.isEmpty {
                    Text(model).tag(model)
                }
            }
        default:
            TextField("Model", text: $model)
        }
    }

    private static let claudeModels = ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-4-5", "claude-fable-5-1"]

    private func loadModel() {
        model = AISettings.current().model
        if provider == .ollama, let chat {
            // Asks the local Ollama server, if it's running
            Task { ollamaModels = await chat.models(for: .ollama) }
        }
    }

    private func saveKey() {
        do {
            try keys.setKey(apiKey, for: provider)
            apiKey = ""
            keyStatus = "Saved in your Keychain."
        } catch {
            keyStatus = error.localizedDescription
        }
    }
}
