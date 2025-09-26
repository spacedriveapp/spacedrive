//
//  SpacedriveApp.swift
//  Spacedrive
//
//  Created by jamie on 2025-09-26.
//

import SwiftUI
import AppKit

@main
struct SpacedriveApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate

    var body: some Scene {
        // Main companion window
        WindowGroup("Spacedrive") {
            JobCompanionView()
                .withSharedState()
                .customTitleBar(
                    center: Text("Jamie's Library")
                        .font(.headline)
                        .foregroundColor(.white),
                    right: HStack(spacing: 8) {
                        Button(action: {}) {
                            Image(systemName: "gear")
                                .foregroundColor(.white)
                        }
                        .buttonStyle(PlainButtonStyle())

                        Button(action: {}) {
                            Image(systemName: "person.circle")
                                .foregroundColor(.white)
                        }
                        .buttonStyle(PlainButtonStyle())
                    }
                )
        }
        .windowStyle(.titleBar)
        .windowResizability(.contentSize)
        .defaultSize(width: 400, height: 600)

        // Browser window
        WindowGroup("Browser") {
            BrowserView()
                .withSharedState()
                .customTitleBar(
                    center: HStack(spacing: 12) {
                        Image(systemName: "folder")
                            .foregroundColor(.white)
                        Text("File Browser")
                            .font(.headline)
                            .foregroundColor(.white)
                    },
                    right: HStack(spacing: 8) {
                        Button(action: {}) {
                            Image(systemName: "magnifyingglass")
                                .foregroundColor(.white)
                        }
                        .buttonStyle(PlainButtonStyle())

                        Button(action: {}) {
                            Image(systemName: "list.bullet")
                                .foregroundColor(.white)
                        }
                        .buttonStyle(PlainButtonStyle())
                    }
                )
        }
        .windowStyle(.titleBar)
        .windowResizability(.contentSize)
        .defaultSize(width: 1200, height: 800)

        // Icon Showcase window
        WindowGroup("Icon Showcase") {
            IconShowcaseView()
                .withSharedState()
                .nativeTrafficLights()
        }
        .windowStyle(.titleBar)
        .windowResizability(.contentSize)
        .defaultSize(width: 1000, height: 800)

        // Inspector window
        WindowGroup("Inspector") {
            InspectorView()
                .withSharedState()
                .nativeTrafficLights()
        }
        .windowStyle(.titleBar)
        .windowResizability(.contentSize)
        .defaultSize(width: 500, height: 700)

        // Settings window
        Settings {
            SettingsView()
                .withSharedState()
        }
    }
}

class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_: Notification) {
        print("Spacedrive for macOS")

        // Configure app as foreground application
        NSApp.setActivationPolicy(.regular)

        setupMenuBar()
        initializeAppState()
    }

    func applicationShouldTerminateAfterLastWindowClosed(_: NSApplication) -> Bool {
        return true
    }

    func applicationShouldHandleReopen(_: NSApplication, hasVisibleWindows _: Bool) -> Bool {
        // When clicking on dock icon or app, activate and show window
        NSApp.activate(ignoringOtherApps: true)
        return true
    }

    @MainActor
    private func setupMenuBar() {
        MenuBarManager.shared.setupMenuBar()
    }

    @MainActor
    private func initializeAppState() {
        // Initialize shared app state
        SharedAppState.shared.initializeDaemonConnection()

        // Apply development window configuration
        applyDevWindowConfiguration()
    }

    @MainActor
    private func applyDevWindowConfiguration() {
        // Set the development window configuration
        SharedAppState.shared.setDevWindowConfiguration(currentDevConfiguration)

        // Open windows based on the configuration
        if currentDevConfiguration.shouldAutoOpen {
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
                SharedAppState.shared.openWindowsForCurrentDevConfiguration()
            }
        }

        print("🔧 Development window configuration set to: \(currentDevConfiguration.displayName)")
    }
}
