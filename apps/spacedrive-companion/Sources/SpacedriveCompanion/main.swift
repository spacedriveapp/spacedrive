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
        print("🚀 Spacedrive Companion app launched!")

        // Configure app as foreground application
        NSApp.setActivationPolicy(.regular)

        setupMenuBar()
        setupWindow()
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        return true
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        // When clicking on dock icon or app, activate and show window
        NSApp.activate(ignoringOtherApps: true)
        if let window = window {
            window.makeKeyAndOrderFront(nil)
        }
        return true
    }

    func applicationDidBecomeActive(_ notification: Notification) {
        // Ensure window is visible when app becomes active
        if let window = window {
            window.makeKeyAndOrderFront(nil)
        }
    }

    @MainActor
    private func setupMenuBar() {
        MenuBarManager.shared.setupMenuBar()
    }

    @MainActor
    private func setupWindow() {
        // Initialize shared app state
        SharedAppState.shared.initializeDaemonConnection()

        // Create the companion window using WindowManager
        window = WindowManager.shared.createWindow(type: .companion, id: "main-companion") {
            JobCompanionView()
                .withSharedState()
        }

        window?.makeKeyAndOrderFront(nil)

        // Force app to become active and show menu bar
        DispatchQueue.main.async {
            NSApp.activate(ignoringOtherApps: true)
            self.window?.makeKeyAndOrderFront(nil)
        }
    }
}

// Main entry point
SpacedriveCompanionApp.main()
