// The Swift API is generated from crates/escrituras-ffi into Generated/ by
// scripts/build-xcframework.sh. Swift-only conveniences go in this module.

import Foundation

extension Library {
    /// Open the library with scripture data from `dataRoot` (a directory
    /// containing `lds-scriptures-2020.12.08/` and optionally `data/`) and the
    /// study database at `studyDatabase`, creating it if needed.
    public static func open(dataRoot: URL, studyDatabase: URL?) throws -> Library {
        try open(dataRoots: [dataRoot.path], studyDbPath: studyDatabase?.path)
    }
}
