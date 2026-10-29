import EscriturasKit
import SwiftUI

/// The main window: books, chapters and the reader, with search in the
/// toolbar and study tools in the inspector
struct ContentView: View {
    @Bindable var model: AppModel
    let chat: ChatModel
    @State private var columnVisibility = NavigationSplitViewVisibility.all
    @FocusState private var focus: Pane?
    @State private var columns = ColumnFocus()
    @State private var chapterColumnWidth: CGFloat = 0

    /// Below this width, showing the inspector hides the sidebar (as Notes and
    /// Mail do) so the reader keeps a comfortable width
    private let roomyWidth: CGFloat = 1240

    var body: some View {
        GeometryReader { geometry in
            splitView
                .onChange(of: model.showInspector) {
                    if model.showInspector && geometry.size.width < roomyWidth {
                        columnVisibility = .doubleColumn
                    }
                }
                .onAppear {
                    if model.showInspector && geometry.size.width < roomyWidth {
                        columnVisibility = .doubleColumn
                    }
                }
        }
    }

    /// Before macOS 26 the toolbar is an opaque band over the columns
    private var legacyToolbar: Bool {
        if #available(macOS 26, *) { return false }
        return true
    }

    /// Room for the title in the chapter column's part of the toolbar. With
    /// the sidebar hidden, the window controls and sidebar toggle come first.
    private var titleWidth: CGFloat {
        let leading: CGFloat = columnVisibility == .all ? 20 : 141
        return max(60, chapterColumnWidth - leading)
    }

    private var splitView: some View {
        NavigationSplitView(columnVisibility: $columnVisibility) {
            SidebarView(model: model, focus: $focus, columns: columns)
                .navigationSplitViewColumnWidth(min: 170, ideal: 200, max: 300)
        } content: {
            Group {
                if model.searchText.isEmpty {
                    ChapterListView(model: model, focus: $focus, columns: columns)
                } else {
                    SearchResultsView(model: model, focus: $focus, columns: columns)
                }
            }
            .navigationSplitViewColumnWidth(min: 200, ideal: 250, max: 420)
            .background(GeometryReader { geometry in
                Color.clear.onChange(of: geometry.size.width, initial: true) {
                    chapterColumnWidth = geometry.size.width
                }
            })
        } detail: {
            ReaderView(model: model, chat: chat, focus: $focus)
        }
        // As in NetNewsWire, each column runs the full height of the window
        // and the toolbar sits over it. macOS 26 lays toolbars out that way
        // itself, with Liquid Glass controls and a blur where content scrolls
        // under them; earlier systems need the toolbar's background hidden.
        .toolbarBackground(legacyToolbar ? .hidden : .automatic, for: .windowToolbar)
        // Start in the reader, so ↑/↓ move between verses right away
        .defaultFocus($focus, .reader)
        .onAppear {
            let focus = $focus
            columns.focusReader = { focus.wrappedValue = .reader }
            columns.readerIsFocused = { focus.wrappedValue == .reader }
            columns.startWatchingKeys(model: model)
        }
        .searchable(text: $model.searchText, placement: .toolbar, prompt: "Search the scriptures")
        .onChange(of: model.searchText) {
            model.scheduleSearch()
        }
        // As in Mail and Notes: Return opens the first result and moves on to
        // the results, where ↑/↓ open each one
        .onSubmit(of: .search) {
            Task {
                await model.submitSearch()
                columns.move(to: .chapters)
            }
        }
        .onChange(of: model.searchFocusRequests) {
            columns.focusSearchField()
        }
        .inspector(isPresented: $model.showInspector) {
            InspectorView(model: model, chat: chat)
                .inspectorColumnWidth(min: 250, ideal: 300, max: 480)
        }
        // The title sits over the chapter column and, as in NetNewsWire and
        // Mail, names what that column lists; the reader carries its own
        // heading. The window's own title would run past the column's edge
        // when the sidebar is hidden, so a toolbar item that truncates to the
        // column stands in for it. The blank title stays: it is what keeps
        // the reader's items at the trailing edge of the toolbar.
        .navigationTitle("")
        .toolbar {
            if #available(macOS 26, *) {
                // A title, not a control: no glass behind it
                ToolbarItem(placement: .navigation) {
                    ColumnTitle(title: model.columnTitle, subtitle: model.columnSubtitle, maxWidth: titleWidth)
                }
                .sharedBackgroundVisibility(.hidden)
            } else {
                ToolbarItem(placement: .navigation) {
                    ColumnTitle(title: model.columnTitle, subtitle: model.columnSubtitle, maxWidth: titleWidth)
                }
            }
        }
        .toolbar {
            // Back and Forward, through the reading history, are in the Go menu
            // (⌘[ and ⌘]); a second pair of arrows here would only confuse
            ToolbarItemGroup(placement: .primaryAction) {
                Button {
                    model.goToPreviousChapter()
                } label: {
                    Label("Previous Chapter", systemImage: "chevron.left")
                }
                .help("Previous chapter")
                .disabled(model.previousChapter == nil)

                Button {
                    model.goToNextChapter()
                } label: {
                    Label("Next Chapter", systemImage: "chevron.right")
                }
                .help("Next chapter")
                .disabled(model.nextChapter == nil)

                Toggle(isOn: $model.showInspector) {
                    Label("Study Tools", systemImage: "sidebar.trailing")
                }
                .toggleStyle(.button)
                .help(model.showInspector ? "Hide study tools" : "Show saved verses, notes and AI chat")
            }
        }
        .sheet(isPresented: $model.showGoToReference) {
            GoToReferenceView(model: model)
        }
        .sheet(item: $model.noteEditorVerse) { verse in
            NoteEditorView(model: model, verse: verse)
        }
        .alert(
            "Something went wrong",
            isPresented: Binding(
                get: { model.errorMessage != nil },
                set: { if !$0 { model.errorMessage = nil } }
            )
        ) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(model.errorMessage ?? "")
        }
    }
}

/// The chapter column's title and subtitle, truncating when the column is narrow
struct ColumnTitle: View {
    let title: String
    let subtitle: String
    let maxWidth: CGFloat

    var body: some View {
        VStack(alignment: .leading, spacing: 1) {
            Text(title)
                .font(.headline)
            if !subtitle.isEmpty {
                Text(subtitle)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
        }
        .lineLimit(1)
        .truncationMode(.tail)
        .frame(maxWidth: maxWidth, alignment: .leading)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isHeader)
    }
}
