import EscriturasKit
import SwiftUI

/// The main window: books, chapters and the reader, with search in the
/// toolbar and study tools in the inspector
struct ContentView: View {
    @Bindable var model: AppModel
    let chat: ChatModel

    var body: some View {
        NavigationSplitView {
            SidebarView(model: model)
                .navigationSplitViewColumnWidth(min: 180, ideal: 220, max: 320)
        } content: {
            Group {
                if model.searchText.isEmpty {
                    ChapterListView(model: model)
                } else {
                    SearchResultsView(model: model)
                }
            }
            .navigationSplitViewColumnWidth(min: 160, ideal: 240, max: 420)
        } detail: {
            ReaderView(model: model, chat: chat)
        }
        .searchable(text: $model.searchText, placement: .toolbar, prompt: "Search the scriptures")
        .onChange(of: model.searchText) {
            model.scheduleSearch()
        }
        .inspector(isPresented: $model.showInspector) {
            InspectorView(model: model, chat: chat)
                .inspectorColumnWidth(min: 260, ideal: 340, max: 520)
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
                        Label("Previous Chapter", systemImage: "arrow.up")
                    }
                    .help("Previous chapter")
                    .disabled(model.previousChapter == nil)

                    Button {
                        model.goToNextChapter()
                    } label: {
                        Label("Next Chapter", systemImage: "arrow.down")
                    }
                    .help("Next chapter")
                    .disabled(model.nextChapter == nil)
                }

                Button {
                    model.showInspector.toggle()
                } label: {
                    Label("Study", systemImage: "sidebar.trailing")
                }
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
