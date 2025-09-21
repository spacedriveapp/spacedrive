import SwiftUI
import AppKit

struct SpacedriveCompanionApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate

    var body: some Scene {
        Settings {
            EmptyView()
        }
    }
}

class AppDelegate: NSObject, NSApplicationDelegate {
    var window: NSWindow?
    var jobListViewModel: JobListViewModel?

    func applicationDidFinishLaunching(_ notification: Notification) {
        setupWindow()
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        return true
    }

    private func setupWindow() {
        let contentView = ContentView()

        // Create the translucent window
        window = TranslucentWindow(
            contentRect: NSRect(x: 100, y: 100, width: 400, height: 600),
            styleMask: [.titled, .closable, .resizable],
            backing: .buffered,
            defer: false
        )

        window?.title = "Spacedrive"
        window?.contentView = NSHostingView(rootView: contentView)
        window?.makeKeyAndOrderFront(nil)
        window?.level = .floating

        // Keep window on top
        window?.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
    }
}

// Main entry point
SpacedriveCompanionApp.main()
