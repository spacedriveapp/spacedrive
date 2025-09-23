import AppKit
import SwiftUI

/// Manages the macOS menu bar for Spacedrive
@MainActor
class MenuBarManager: ObservableObject {
    static let shared = MenuBarManager()

    private init() {}

    func setupMenuBar() {
        let mainMenu = NSMenu()
        NSApp.mainMenu = mainMenu

        // App Menu (Spacedrive)
        setupAppMenu(mainMenu)

        // Daemon Menu
        setupDaemonMenu(mainMenu)

        // File Menu
        setupFileMenu(mainMenu)

        // Edit Menu
        setupEditMenu(mainMenu)

        // View Menu
        setupViewMenu(mainMenu)

        // Window Menu
        setupWindowMenu(mainMenu)

        // Help Menu
        setupHelpMenu(mainMenu)
    }

    // MARK: - App Menu (Spacedrive)
    private func setupAppMenu(_ mainMenu: NSMenu) {
        let appMenuItem = NSMenuItem()
        mainMenu.addItem(appMenuItem)

        let appMenu = NSMenu()
        appMenuItem.submenu = appMenu

        // About Spacedrive
        let aboutItem = NSMenuItem(
            title: "About Spacedrive",
            action: #selector(showAbout),
            keyEquivalent: ""
        )
        aboutItem.target = self
        appMenu.addItem(aboutItem)

        appMenu.addItem(NSMenuItem.separator())

        // Preferences
        let preferencesItem = NSMenuItem(
            title: "Preferences...",
            action: #selector(showPreferences),
            keyEquivalent: ","
        )
        preferencesItem.target = self
        appMenu.addItem(preferencesItem)

        appMenu.addItem(NSMenuItem.separator())

        // Services
        let servicesMenuItem = NSMenuItem(title: "Services", action: nil, keyEquivalent: "")
        let servicesMenu = NSMenu()
        servicesMenuItem.submenu = servicesMenu
        appMenu.addItem(servicesMenuItem)
        NSApp.servicesMenu = servicesMenu

        appMenu.addItem(NSMenuItem.separator())

        // Hide Spacedrive
        let hideItem = NSMenuItem(
            title: "Hide Spacedrive",
            action: #selector(NSApp.hide(_:)),
            keyEquivalent: "h"
        )
        appMenu.addItem(hideItem)

        // Hide Others
        let hideOthersItem = NSMenuItem(
            title: "Hide Others",
            action: #selector(NSApp.hideOtherApplications(_:)),
            keyEquivalent: "h"
        )
        hideOthersItem.keyEquivalentModifierMask = [.command, .option]
        appMenu.addItem(hideOthersItem)

        // Show All
        let showAllItem = NSMenuItem(
            title: "Show All",
            action: #selector(NSApp.unhideAllApplications(_:)),
            keyEquivalent: ""
        )
        appMenu.addItem(showAllItem)

        appMenu.addItem(NSMenuItem.separator())

        // Quit Spacedrive
        let quitItem = NSMenuItem(
            title: "Quit Spacedrive",
            action: #selector(NSApp.terminate(_:)),
            keyEquivalent: "q"
        )
        appMenu.addItem(quitItem)
    }

    // MARK: - Daemon Menu
    private func setupDaemonMenu(_ mainMenu: NSMenu) {
        let daemonMenuItem = NSMenuItem(title: "Daemon", action: nil, keyEquivalent: "")
        mainMenu.addItem(daemonMenuItem)

        let daemonMenu = NSMenu(title: "Daemon")
        daemonMenuItem.submenu = daemonMenu

        // Connection Status (disabled item showing current status)
        let statusItem = NSMenuItem(
            title: "Status: Disconnected",
            action: nil,
            keyEquivalent: ""
        )
        statusItem.isEnabled = false
        daemonMenu.addItem(statusItem)

        daemonMenu.addItem(NSMenuItem.separator())

        // Connect to Daemon
        let connectItem = NSMenuItem(
            title: "Connect to Daemon",
            action: #selector(connectToDaemon),
            keyEquivalent: ""
        )
        connectItem.target = self
        daemonMenu.addItem(connectItem)

        // Disconnect from Daemon
        let disconnectItem = NSMenuItem(
            title: "Disconnect from Daemon",
            action: #selector(disconnectFromDaemon),
            keyEquivalent: ""
        )
        disconnectItem.target = self
        disconnectItem.isEnabled = false
        daemonMenu.addItem(disconnectItem)

        daemonMenu.addItem(NSMenuItem.separator())

        // Refresh Jobs
        let refreshItem = NSMenuItem(
            title: "Refresh Jobs",
            action: #selector(refreshJobs),
            keyEquivalent: "r"
        )
        refreshItem.target = self
        daemonMenu.addItem(refreshItem)

        // Store references for dynamic updates
        daemonMenu.items.forEach { item in
            switch item.title {
            case "Status: Disconnected", "Status: Connected", "Status: Connecting":
                item.representedObject = "status"
            case "Connect to Daemon":
                item.representedObject = "connect"
            case "Disconnect from Daemon":
                item.representedObject = "disconnect"
            default:
                break
            }
        }

        // Store menu reference for updates
        self.daemonMenu = daemonMenu
    }

    private var daemonMenu: NSMenu?

    // MARK: - File Menu
    private func setupFileMenu(_ mainMenu: NSMenu) {
        let fileMenuItem = NSMenuItem(title: "File", action: nil, keyEquivalent: "")
        mainMenu.addItem(fileMenuItem)

        let fileMenu = NSMenu(title: "File")
        fileMenuItem.submenu = fileMenu

        // New Window
        let newWindowItem = NSMenuItem(
            title: "New Browser Window",
            action: #selector(newBrowserWindow),
            keyEquivalent: "n"
        )
        newWindowItem.target = self
        fileMenu.addItem(newWindowItem)

        fileMenu.addItem(NSMenuItem.separator())

        // Close Window
        let closeItem = NSMenuItem(
            title: "Close Window",
            action: #selector(performClose),
            keyEquivalent: "w"
        )
        closeItem.target = self
        fileMenu.addItem(closeItem)
    }

    // MARK: - Edit Menu
    private func setupEditMenu(_ mainMenu: NSMenu) {
        let editMenuItem = NSMenuItem(title: "Edit", action: nil, keyEquivalent: "")
        mainMenu.addItem(editMenuItem)

        let editMenu = NSMenu(title: "Edit")
        editMenuItem.submenu = editMenu

        // Standard edit items
        editMenu.addItem(NSMenuItem(title: "Undo", action: Selector(("undo:")), keyEquivalent: "z"))
        editMenu.addItem(NSMenuItem(title: "Redo", action: Selector(("redo:")), keyEquivalent: "Z"))
        editMenu.addItem(NSMenuItem.separator())
        editMenu.addItem(NSMenuItem(title: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x"))
        editMenu.addItem(NSMenuItem(title: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c"))
        editMenu.addItem(NSMenuItem(title: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v"))
        editMenu.addItem(NSMenuItem(title: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a"))
    }

    // MARK: - View Menu
    private func setupViewMenu(_ mainMenu: NSMenu) {
        let viewMenuItem = NSMenuItem(title: "View", action: nil, keyEquivalent: "")
        mainMenu.addItem(viewMenuItem)

        let viewMenu = NSMenu(title: "View")
        viewMenuItem.submenu = viewMenu

        // Show Job Monitor
        let jobMonitorItem = NSMenuItem(
            title: "Show Job Monitor",
            action: #selector(showJobMonitor),
            keyEquivalent: "j"
        )
        jobMonitorItem.target = self
        viewMenu.addItem(jobMonitorItem)

        viewMenu.addItem(NSMenuItem.separator())

        // Enter Full Screen
        let fullScreenItem = NSMenuItem(
            title: "Enter Full Screen",
            action: #selector(toggleFullScreen),
            keyEquivalent: "f"
        )
        fullScreenItem.keyEquivalentModifierMask = [.command, .control]
        fullScreenItem.target = self
        viewMenu.addItem(fullScreenItem)
    }

    // MARK: - Window Menu
    private func setupWindowMenu(_ mainMenu: NSMenu) {
        let windowMenuItem = NSMenuItem(title: "Window", action: nil, keyEquivalent: "")
        mainMenu.addItem(windowMenuItem)

        let windowMenu = NSMenu(title: "Window")
        windowMenuItem.submenu = windowMenu
        NSApp.windowsMenu = windowMenu

        // Minimize
        windowMenu.addItem(NSMenuItem(title: "Minimize", action: #selector(NSWindow.performMiniaturize(_:)), keyEquivalent: "m"))

        // Zoom
        windowMenu.addItem(NSMenuItem(title: "Zoom", action: #selector(NSWindow.performZoom(_:)), keyEquivalent: ""))

        windowMenu.addItem(NSMenuItem.separator())

        // Bring All to Front
        windowMenu.addItem(NSMenuItem(title: "Bring All to Front", action: #selector(NSApp.arrangeInFront(_:)), keyEquivalent: ""))
    }

    // MARK: - Help Menu
    private func setupHelpMenu(_ mainMenu: NSMenu) {
        let helpMenuItem = NSMenuItem(title: "Help", action: nil, keyEquivalent: "")
        mainMenu.addItem(helpMenuItem)

        let helpMenu = NSMenu(title: "Help")
        helpMenuItem.submenu = helpMenu
        NSApp.helpMenu = helpMenu

        let helpItem = NSMenuItem(
            title: "Spacedrive Help",
            action: #selector(showHelp),
            keyEquivalent: "?"
        )
        helpItem.target = self
        helpMenu.addItem(helpItem)
    }

    // MARK: - Menu Actions
    @objc private func showAbout() {
        NSApp.orderFrontStandardAboutPanel(nil)
    }

    @objc private func showPreferences() {
        SharedAppState.shared.dispatch(.openWindow(.settings, nil))
    }

    @objc private func connectToDaemon() {
        SharedAppState.shared.dispatch(.connectToDaemon)
    }

    @objc private func disconnectFromDaemon() {
        SharedAppState.shared.dispatch(.disconnectFromDaemon)
    }

    @objc private func refreshJobs() {
        SharedAppState.shared.dispatch(.refreshJobs)
    }

    @objc private func newBrowserWindow() {
        SharedAppState.shared.dispatch(.openWindow(.browser, nil))
    }

    @objc private func showJobMonitor() {
        SharedAppState.shared.dispatch(.openWindow(.companion, nil))
    }

    @objc private func performClose() {
        NSApp.keyWindow?.performClose(nil)
    }

    @objc private func toggleFullScreen() {
        NSApp.keyWindow?.toggleFullScreen(nil)
    }

    @objc private func showHelp() {
        if let url = URL(string: "https://spacedrive.com/docs") {
            NSWorkspace.shared.open(url)
        }
    }

    // MARK: - Dynamic Menu Updates
    func updateDaemonMenuStatus(_ status: ConnectionStatus) {
        guard let daemonMenu = daemonMenu else { return }

        for item in daemonMenu.items {
            guard let representedObject = item.representedObject as? String else { continue }

            switch representedObject {
            case "status":
                item.title = "Status: \(status.displayName)"
            case "connect":
                item.isEnabled = status != .connected && status != .connecting
            case "disconnect":
                item.isEnabled = status == .connected
            default:
                break
            }
        }
    }
}

// ConnectionStatus displayName is already defined in JobModels.swift
