import Foundation

/// Where the app finds scripture data and keeps the study database.
public enum DataLocation {
    /// Folder (relative to a data root) holding the scripture JSON
    static let scriptureFolder = "lds-scriptures-2020.12.08"

    /// Directories that may contain the scripture data, in priority order:
    ///
    /// 1. The `DataRoot` user default (e.g. `-DataRoot /path` on the command line)
    /// 2. The app bundle's Resources (a packaged app)
    /// 3. A checkout of the repository above the executable or the current
    ///    directory (running with `swift run` or from Xcode)
    /// 4. `~/Library/Application Support/escrituras` (installed by install.sh)
    public static func candidateRoots(
        bundle: Bundle = .main,
        defaults: UserDefaults = .standard,
        fileManager: FileManager = .default
    ) -> [URL] {
        var roots: [URL] = []
        if let configured = defaults.string(forKey: "DataRoot") {
            roots.append(URL(fileURLWithPath: configured))
        }
        if let resources = bundle.resourceURL {
            roots.append(resources)
        }
        let currentDirectory = URL(fileURLWithPath: fileManager.currentDirectoryPath)
        for start in [bundle.bundleURL, currentDirectory] {
            if let checkout = repositoryRoot(above: start, fileManager: fileManager) {
                roots.append(checkout)
            }
        }
        roots.append(supportDirectory(fileManager: fileManager))
        return roots
    }

    /// The nearest directory at or above `url` that contains the scripture data
    static func repositoryRoot(above url: URL, fileManager: FileManager = .default) -> URL? {
        var directory = url.standardizedFileURL
        for _ in 0..<12 {
            let candidate = directory.appendingPathComponent(scriptureFolder)
            if fileManager.fileExists(atPath: candidate.path) {
                return directory
            }
            let parent = directory.deletingLastPathComponent()
            if parent.path == directory.path { break }
            directory = parent
        }
        return nil
    }

    /// `~/Library/Application Support/escrituras`, the same directory the
    /// terminal app uses, so saved verses and notes are shared between them
    public static func supportDirectory(fileManager: FileManager = .default) -> URL {
        let base = fileManager.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? URL(fileURLWithPath: NSHomeDirectory()).appendingPathComponent("Library/Application Support")
        return base.appendingPathComponent("escrituras")
    }

    /// The study database shared with the terminal app
    public static func studyDatabase(fileManager: FileManager = .default) -> URL {
        supportDirectory(fileManager: fileManager).appendingPathComponent("study.db")
    }
}
