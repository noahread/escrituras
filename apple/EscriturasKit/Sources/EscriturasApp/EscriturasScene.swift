import AppKit
import EscriturasKit
import Observation
import SwiftUI

/// Loads the scripture data and owns the models once it's ready
@Observable
@MainActor
public final class AppLauncher {
    public enum Phase {
        case loading
        case ready(AppModel, ChatModel)
        case failed(String)
    }

    public private(set) var phase: Phase = .loading

    public var model: AppModel? {
        if case .ready(let model, _) = phase { return model }
        return nil
    }

    public var chat: ChatModel? {
        if case .ready(_, let chat) = phase { return chat }
        return nil
    }

    // nonisolated so it can be created as a scene's @State
    public nonisolated init() {}

    public func start() {
        guard case .loading = phase else { return }
        let roots = DataLocation.candidateRoots().map(\.path)
        let database = DataLocation.studyDatabase().path
        Task {
            // Parsing the scripture data takes a moment; keep the window responsive
            let result = await Task.detached(priority: .userInitiated) {
                Result { try Library.open(dataRoots: roots, studyDbPath: database) }
            }.value
            switch result {
            case .success(let library):
                let model = AppModel(library: library)
                model.openStartingChapter()
                LaunchOptions.apply(to: model)
                phase = .ready(model, ChatModel(backend: LibraryChatBackend(library: library), library: library))
            case .failure(let error):
                phase = .failed(ChatModel.describe(error))
            }
        }
    }
}

/// Optional launch arguments, mainly for screenshots and debugging, e.g.
/// `Escrituras -Open "Alma 32:21" -ShowInspector YES`
enum LaunchOptions {
    @MainActor
    static func apply(to model: AppModel, defaults: UserDefaults = .standard) {
        if let reference = defaults.string(forKey: "Open") {
            model.goTo(reference: reference)
        }
        if let query = defaults.string(forKey: "Search") {
            model.searchText = query
            model.scheduleSearch(delay: .zero)
        }
        if defaults.bool(forKey: "ShowInspector") {
            model.showInspector = true
        }
        if let tab = defaults.string(forKey: "InspectorTab").flatMap(AppModel.InspectorTab.init(rawValue:)) {
            model.inspectorTab = tab
        }
        // Opens the note editor on the verse selected by -Open
        if defaults.bool(forKey: "EditNote") {
            model.noteEditorVerse = model.selectedVerseValue
        }
        if defaults.bool(forKey: "GoToReference") {
            model.showGoToReference = true
        }
    }
}

/// The app's scenes: the main window and Settings
public struct EscriturasScene: Scene {
    @State private var launcher = AppLauncher()

    public init() {}

    public var body: some Scene {
        Window("Escrituras", id: "main") {
            RootView(launcher: launcher)
                .frame(minWidth: 820, minHeight: 520)
                .background(FitWindowToScreen())
                .task { launcher.start() }
        }
        .defaultSize(width: 1180, height: 760)
        .commands {
            AppCommands(launcher: launcher)
        }

        Settings {
            SettingsView(chat: launcher.chat)
        }
    }
}

/// Makes the app a regular foreground app when run with `swift run`,
/// and quits when the window closes
public final class EscriturasAppDelegate: NSObject, NSApplicationDelegate {
    public func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        // -Appearance dark|light overrides the system appearance, e.g. for screenshots
        switch UserDefaults.standard.string(forKey: "Appearance") {
        case "dark": NSApp.appearance = NSAppearance(named: .darkAqua)
        case "light": NSApp.appearance = NSAppearance(named: .aqua)
        default: break
        }
    }

    public func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }
}

struct RootView: View {
    let launcher: AppLauncher

    var body: some View {
        switch launcher.phase {
        case .loading:
            ProgressView("Loading the scriptures…")
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .failed(let message):
            ContentUnavailableView {
                Label("Scripture Data Not Found", systemImage: "book.closed")
            } description: {
                Text(message)
            }
        case .ready(let model, let chat):
            ContentView(model: model, chat: chat)
                .modifier(OpenSettingsAtLaunch())
        }
    }
}

/// `-OpenSettings YES` opens the Settings window once the app is ready
private struct OpenSettingsAtLaunch: ViewModifier {
    @Environment(\.openSettings) private var openSettings

    func body(content: Content) -> some View {
        content.task {
            if UserDefaults.standard.bool(forKey: "OpenSettings") {
                openSettings()
            }
        }
    }
}

/// Shrinks the window to the screen's visible area when it first opens, so the
/// default size never leaves it partly off a small display
private struct FitWindowToScreen: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView {
        FittingView()
    }

    func updateNSView(_ nsView: NSView, context: Context) {}

    private final class FittingView: NSView {
        private var fitted = false

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            guard !fitted, let window, let screen = window.screen ?? NSScreen.main else { return }
            fitted = true
            let visible = screen.visibleFrame
            var frame = window.frame
            frame.size.width = min(frame.width, visible.width)
            frame.size.height = min(frame.height, visible.height)
            frame.origin.x = min(max(frame.minX, visible.minX), visible.maxX - frame.width)
            frame.origin.y = min(max(frame.minY, visible.minY), visible.maxY - frame.height)
            if frame != window.frame {
                window.setFrame(frame, display: true)
            }
        }
    }
}
