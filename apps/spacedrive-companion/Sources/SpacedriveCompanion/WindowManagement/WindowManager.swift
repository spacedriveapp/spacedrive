import SwiftUI
import AppKit

/// Window Types for Spacedrive
enum WindowType: String, CaseIterable {
    case companion = "companion"
    case browser = "browser"
    case settings = "settings"
    case explorer = "explorer"
    case overview = "overview"

    var defaultSize: NSSize {
        switch self {
        case .companion:
            return NSSize(width: 400, height: 600)
        case .browser:
            return NSSize(width: 1200, height: 800)
        case .settings:
            return NSSize(width: 800, height: 600)
        case .explorer:
            return NSSize(width: 1000, height: 700)
        case .overview:
            return NSSize(width: 900, height: 600)
        }
    }

    var minSize: NSSize {
        switch self {
        case .companion:
            return NSSize(width: 300, height: 400)
        case .browser:
            return NSSize(width: 800, height: 500)
        case .settings:
            return NSSize(width: 600, height: 400)
        case .explorer:
            return NSSize(width: 700, height: 500)
        case .overview:
            return NSSize(width: 600, height: 400)
        }
    }

    var isResizable: Bool {
        switch self {
        case .companion:
            return true
        case .browser, .settings, .explorer, .overview:
            return true
        }
    }

    var isFloating: Bool {
        switch self {
        case .companion:
            return false // Changed to show menu bar when focused
        case .browser, .settings, .explorer, .overview:
            return false
        }
    }
}

/// Window Manager - Handles multi-window state and coordination
@MainActor
class WindowManager: ObservableObject {
    static let shared = WindowManager()

    @Published private(set) var windows: [String: NSWindow] = [:]
    @Published private(set) var windowStates: [String: WindowState] = [:]

    private init() {}

    // MARK: - Window Creation
    func createWindow<Content: View>(
        type: WindowType,
        id: String? = nil,
        content: @escaping () -> Content
    ) -> NSWindow {
        let windowId = id ?? UUID().uuidString

        let window = RoundedWindow(
            contentRect: NSRect(origin: .zero, size: type.defaultSize),
            styleMask: [.borderless, .resizable],
            backing: .buffered,
            defer: false
        )

        // Configure window based on type
        window.minSize = type.minSize
        window.level = type.isFloating ? .floating : .normal
        window.title = "Spacedrive"

        // Create content with shared environment
        let contentView = content()
            .environment(\.window, window)
            .environment(\.windowType, type)
            .environment(\.windowId, windowId)
            .environmentObject(SharedAppState.shared)

        window.contentView = NSHostingView(rootView: contentView)

        // Store window reference
        windows[windowId] = window
        windowStates[windowId] = WindowState(id: windowId, type: type, window: window)

        // Window lifecycle callbacks
        setupWindowCallbacks(window: window, id: windowId)

        return window
    }

    // MARK: - Window Management
    func showWindow(type: WindowType, id: String? = nil) {
        let windowId = id ?? type.rawValue

        if let existingWindow = windows[windowId] {
            existingWindow.makeKeyAndOrderFront(nil)
            return
        }

        // Create window based on type
        let window: NSWindow
        switch type {
        case .companion:
            window = createWindow(type: .companion, id: windowId) {
                JobCompanionView()
            }
        case .browser:
            window = createWindow(type: .browser, id: windowId) {
                BrowserView()
            }
        case .settings:
            window = createWindow(type: .settings, id: windowId) {
                SettingsView()
            }
        case .explorer:
            window = createWindow(type: .explorer, id: windowId) {
                ExplorerView()
            }
        case .overview:
            window = createWindow(type: .overview, id: windowId) {
                OverviewView()
            }
        }

        window.makeKeyAndOrderFront(nil)
    }

    func closeWindow(id: String) {
        if let window = windows[id] {
            window.close()
        }
    }

    func closeAllWindows() {
        windows.values.forEach { $0.close() }
    }

    // MARK: - Private Methods
    private func setupWindowCallbacks(window: NSWindow, id: String) {
        // Window will close callback
        let observer = NotificationCenter.default.addObserver(
            forName: NSWindow.willCloseNotification,
            object: window,
            queue: .main
        ) { [weak self] _ in
            DispatchQueue.main.async {
                self?.windows.removeValue(forKey: id)
                self?.windowStates.removeValue(forKey: id)
            }
        }

        // Store observer for cleanup
        windowStates[id]?.notificationObserver = observer
    }
}

/// Window State - Tracks individual window state
class WindowState: ObservableObject {
    let id: String
    let type: WindowType
    weak var window: NSWindow?
    var notificationObserver: NSObjectProtocol?

    @Published var isKeyWindow = false
    @Published var isVisible = false

    init(id: String, type: WindowType, window: NSWindow) {
        self.id = id
        self.type = type
        self.window = window

        // Setup state tracking
        setupStateTracking()
    }

    private func setupStateTracking() {
        // Track key window status
        NotificationCenter.default.addObserver(
            forName: NSWindow.didBecomeKeyNotification,
            object: window,
            queue: .main
        ) { [weak self] _ in
            self?.isKeyWindow = true
        }

        NotificationCenter.default.addObserver(
            forName: NSWindow.didResignKeyNotification,
            object: window,
            queue: .main
        ) { [weak self] _ in
            self?.isKeyWindow = false
        }
    }

    deinit {
        if let observer = notificationObserver {
            NotificationCenter.default.removeObserver(observer)
        }
    }
}

// MARK: - Environment Keys
private struct WindowTypeEnvironmentKey: EnvironmentKey {
    static let defaultValue: WindowType = .companion
}

private struct WindowIdEnvironmentKey: EnvironmentKey {
    static let defaultValue: String = ""
}

extension EnvironmentValues {
    var windowType: WindowType {
        get { self[WindowTypeEnvironmentKey.self] }
        set { self[WindowTypeEnvironmentKey.self] = newValue }
    }

    var windowId: String {
        get { self[WindowIdEnvironmentKey.self] }
        set { self[WindowIdEnvironmentKey.self] = newValue }
    }
}

// MARK: - Placeholder Views (to be implemented)
struct BrowserView: View {
    var body: some View {
        Text("Browser Window")
            .h2()
    }
}

struct SettingsView: View {
    var body: some View {
        Text("Settings Window")
            .h2()
    }
}

struct ExplorerView: View {
    var body: some View {
        Text("Explorer Window")
            .h2()
    }
}

struct OverviewView: View {
    var body: some View {
        Text("Overview Window")
            .h2()
    }
}
