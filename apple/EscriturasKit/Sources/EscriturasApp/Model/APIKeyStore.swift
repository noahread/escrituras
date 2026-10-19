import EscriturasKit
import Foundation
import Security

/// Stores AI provider API keys in the user's Keychain
public struct APIKeyStore: Sendable {
    public let service: String

    public init(service: String = "com.escrituras.app.api-keys") {
        self.service = service
    }

    public func key(for provider: AiProvider) -> String? {
        var query = baseQuery(for: provider)
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne

        var result: AnyObject?
        guard SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess,
              let data = result as? Data,
              let key = String(data: data, encoding: .utf8),
              !key.isEmpty
        else {
            return nil
        }
        return key
    }

    /// Save `key` for `provider`, replacing any existing key. An empty key deletes it.
    public func setKey(_ key: String, for provider: AiProvider) throws {
        let trimmed = key.trimmingCharacters(in: .whitespacesAndNewlines)
        SecItemDelete(baseQuery(for: provider) as CFDictionary)
        guard !trimmed.isEmpty else { return }

        var item = baseQuery(for: provider)
        item[kSecValueData as String] = Data(trimmed.utf8)
        item[kSecAttrLabel as String] = "Escrituras \(provider.displayName) API key"
        let status = SecItemAdd(item as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw KeychainError(status: status)
        }
    }

    private func baseQuery(for provider: AiProvider) -> [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: provider.storageName,
        ]
    }
}

public struct KeychainError: LocalizedError {
    public let status: OSStatus

    public var errorDescription: String? {
        let message = SecCopyErrorMessageString(status, nil) as String? ?? "error \(status)"
        return "Couldn't save the API key to the Keychain: \(message)"
    }
}
