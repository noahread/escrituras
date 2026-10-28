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
            .navigationSplitViewColumnWidth(min: 160, ideal: 210, max: 400)
        } detail: {
            ReaderView(model: model, chat: chat, focus: $focus)
        }
        // Like Safari and Mail: the sidebar is tinted the full height of the
        // window and the toolbar over the other columns takes their background
        .toolbarBackground(.hidden, for: .windowToolbar)
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
        .toolbar {
            ToolbarItemGroup(placement: .navigation) {
                Button {
                    model.goBack()
                } label: {
                    Label("Back", systemImage: "chevron.backward")
                }
                .help("Go back")
                .disabled(!model.canGoBack)

                Button {
                    model.goForward()
                } label: {
                    Label("Forward", systemImage: "chevron.forward")
                }
                .help("Go forward")
                .disabled(!model.canGoForward)
            }

            ToolbarItemGroup(placement: .primaryAction) {
                ControlGroup {
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
                }

                Toggle(isOn: $model.showInspector) {
                    Label("Study Tools", systemImage: "sidebar.trailing")
                }
                .toggleStyle(.button)
                .help(model.showInspector ? "Hide study tools" : "Show saved verses, notes and AI chat")
            }
        }
        .navigationTitle(model.title)
        .navigationSubtitle(model.volumeOfLocation ?? "")
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
