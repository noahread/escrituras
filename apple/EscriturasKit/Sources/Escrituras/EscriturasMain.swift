import EscriturasApp
import SwiftUI

@main
struct EscriturasMain: App {
    @NSApplicationDelegateAdaptor(EscriturasAppDelegate.self) private var appDelegate

    var body: some Scene {
        EscriturasScene()
    }
}
