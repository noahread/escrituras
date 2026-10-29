import AppKit
import SwiftUI

/// The columns the arrow keys move between, as in NetNewsWire: ← and → change
/// the column, ↑ and ↓ move within it
enum Pane: Hashable {
    case books, chapters, reader
}

/// Moves keyboard focus between the columns with ←, → and Return.
///
/// The keys are watched at the window level: SwiftUI's focus state can't move
/// keyboard focus into a List on macOS 14, and the sidebar's outline view would
/// otherwise take ← and → to collapse and expand its sections. ↑ and ↓ stay
/// with the lists themselves and with the reader's own key handlers.
@MainActor
final class ColumnFocus {
    private struct Anchor {
        weak var view: NSView?
    }

    private var anchors: [Pane: Anchor] = [:]
    /// A column asked for focus before its list was on screen
    private var pending: Pane?
    private var monitor: Any?
    /// Focuses the reader through SwiftUI, which works for its focusable view
    var focusReader: () -> Void = {}
    var readerIsFocused: () -> Bool = { false }

    deinit {
        if let monitor {
            NSEvent.removeMonitor(monitor)
        }
    }

    /// Called by the view placed behind each list
    func register(_ pane: Pane, anchor: NSView) {
        anchors[pane] = Anchor(view: anchor)
        if pending == pane {
            // The list's table view exists once this view is in the window
            DispatchQueue.main.async { self.move(to: pane) }
        }
    }

    func startWatchingKeys(model: AppModel) {
        guard monitor == nil else { return }
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            // Monitors run on the main thread
            let handled = MainActor.assumeIsolated {
                self?.handle(event, model: model) ?? false
            }
            return handled ? nil : event
        }
    }

    /// The column that has keyboard focus
    var current: Pane? {
        if let responder = anchors.values.first?.view?.window?.firstResponder as? NSView {
            for (pane, anchor) in anchors {
                if let table = anchor.view?.nearestTableView(),
                   responder === table || responder.isDescendant(of: table) {
                    return pane
                }
            }
        }
        return readerIsFocused() ? .reader : nil
    }

    /// Move keyboard focus to a column. A list that isn't on screen yet (the
    /// results of a search just submitted, say) gets it once it appears;
    /// `whenShown` waits for that even if a list is there now, for when the
    /// column is about to show a different list.
    func move(to pane: Pane, whenShown: Bool = false) {
        switch pane {
        case .reader:
            pending = nil
            focusReader()
        case .books, .chapters:
            guard !whenShown,
                  let anchor = anchors[pane]?.view,
                  let window = anchor.window,
                  let table = anchor.nearestTableView()
            else {
                pending = pane
                return
            }
            pending = nil
            window.makeFirstResponder(table)
        }
    }

    /// Returns true when the key was used to change columns
    private func handle(_ event: NSEvent, model: AppModel) -> Bool {
        let modifiers = event.modifierFlags.intersection(.deviceIndependentFlagsMask).subtracting([.numericPad, .function])
        guard modifiers.isEmpty,
              let window = anchors.values.first?.view?.window,
              event.window === window,
              !(window.firstResponder is NSTextView), // typing in a text field
              let pane = current
        else { return false }

        // Escape leaves search and returns to the chapter list
        if event.keyCode == 53, !model.searchText.isEmpty {
            model.clearSearch()
            move(to: .chapters, whenShown: true)
            return true
        }

        let key = event.specialKey
        switch (pane, key) {
        case (.books, .rightArrow?), (.books, .carriageReturn?), (.books, .enter?):
            // → or Return opens the book and moves on to its chapters
            guard model.openSelectedBook() else { return false }
            move(to: .chapters)
        case (.chapters, .leftArrow?):
            move(to: .books)
        case (.chapters, .rightArrow?), (.chapters, .carriageReturn?), (.chapters, .enter?):
            guard model.location != nil else { return false }
            move(to: .reader)
        case (.reader, .leftArrow?):
            move(to: .chapters)
        default:
            return false
        }
        return true
    }
}

extension View {
    /// Mark a list as one of the columns the arrow keys move between
    func column(_ pane: Pane, focus: FocusState<Pane?>.Binding, in columns: ColumnFocus) -> some View {
        focused(focus, equals: pane)
            .background(ColumnAnchor(pane: pane, columns: columns))
    }
}

/// Sits behind a list so `ColumnFocus` can find its table view
private struct ColumnAnchor: NSViewRepresentable {
    let pane: Pane
    let columns: ColumnFocus

    func makeNSView(context: Context) -> AnchorView {
        let view = AnchorView()
        // The list's table view can only be found once both are in the window
        view.movedToWindow = { [weak view] in
            if let view { columns.register(pane, anchor: view) }
        }
        return view
    }

    func updateNSView(_ nsView: AnchorView, context: Context) {
        columns.register(pane, anchor: nsView)
    }

    final class AnchorView: NSView {
        var movedToWindow: () -> Void = {}

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            if window != nil {
                movedToWindow()
            }
        }
    }
}

extension NSView {
    /// The table view of the list this view is placed behind: the first one
    /// found under the closest ancestor that contains any
    fileprivate func nearestTableView() -> NSTableView? {
        var ancestor = superview
        while let view = ancestor {
            if let table = view.firstDescendant(NSTableView.self) {
                return table
            }
            ancestor = view.superview
        }
        return nil
    }

    fileprivate func firstDescendant<T: NSView>(_ type: T.Type) -> T? {
        var queue = subviews
        while !queue.isEmpty {
            let view = queue.removeFirst()
            if let match = view as? T { return match }
            queue.append(contentsOf: view.subviews)
        }
        return nil
    }
}
