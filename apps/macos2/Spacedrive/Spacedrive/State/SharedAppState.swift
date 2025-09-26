import Combine
import SwiftUI
import SpacedriveClient

/// Shared Application State - Global state management across all windows
/// Similar to Redux store or React Context, but with Combine/ObservableObject
@MainActor
class SharedAppState: ObservableObject {
    static let shared = SharedAppState()

    // MARK: - Connection State

    @Published var connectionStatus: ConnectionStatus = .disconnected
    @Published var daemonConnector: DaemonConnector?

    // MARK: - User Preferences

    @Published var userPreferences = UserPreferences()

    // MARK: - Jobs State

    @Published var globalJobs: [JobInfo] = []
    @Published var jobsLastUpdated: Date = .init()

    // MARK: - Core Status

    @Published var coreStatus: CoreStatus?

    // MARK: - Library State

    @Published var currentLibrary: LibraryInfo?
    @Published var availableLibraries: [LibraryInfo] = []
    @Published var currentLibraryId: String?

    // MARK: - UI State

    @Published var theme: SpacedriveTheme = .dark
    @Published var sidebarCollapsed: Bool = false

    private var cancellables = Set<AnyCancellable>()

    private init() {
        setupConnections()
        loadUserPreferences()
    }

    // MARK: - Connection Management

    func initializeDaemonConnection() {
        if daemonConnector == nil {
            daemonConnector = DaemonConnector()

            // Subscribe to daemon connector state
            daemonConnector?.$connectionStatus
                .receive(on: DispatchQueue.main)
                .sink { [weak self] status in
                    self?.connectionStatus = status
                    // Update menu bar status
                    MenuBarManager.shared.updateDaemonMenuStatus(status)
                }
                .store(in: &cancellables)

            daemonConnector?.$jobs
                .receive(on: DispatchQueue.main)
                .sink { [weak self] jobs in
                    self?.globalJobs = jobs
                    self?.jobsLastUpdated = Date()
                }
                .store(in: &cancellables)

            daemonConnector?.$availableLibraries
                .receive(on: DispatchQueue.main)
                .sink { [weak self] libraries in
                    self?.availableLibraries = libraries
                    // Update currentLibrary if we have a currentLibraryId but currentLibrary is nil
                    if let currentLibraryId = self?.currentLibraryId,
                       self?.currentLibrary == nil,
                       let library = libraries.first(where: { $0.id == currentLibraryId })
                    {
                        self?.currentLibrary = library
                    }
                }
                .store(in: &cancellables)

            daemonConnector?.$currentLibraryId
                .receive(on: DispatchQueue.main)
                .sink { [weak self] libraryId in
                    self?.currentLibraryId = libraryId
                    if let libraryId = libraryId {
                        self?.currentLibrary = self?.availableLibraries.first { $0.id == libraryId }
                    } else {
                        self?.currentLibrary = nil
                    }
                }
                .store(in: &cancellables)

            daemonConnector?.$coreStatus
                .receive(on: DispatchQueue.main)
                .sink { [weak self] coreStatus in
                    print("🔍 SharedAppState received coreStatus update: \(coreStatus != nil ? "loaded" : "nil")")
                    if let status = coreStatus {
                        print("🔍 Core status services: location_watcher=\(status.services.locationWatcher.running), networking=\(status.services.networking.running), volume_monitor=\(status.services.volumeMonitor.running), file_sharing=\(status.services.fileSharing.running)")
                    }
                    self?.coreStatus = coreStatus
                }
                .store(in: &cancellables)

            daemonConnector?.connect()
        }
    }

    func disconnectDaemon() {
        daemonConnector?.disconnect()
        connectionStatus = .disconnected
    }

    // MARK: - Library Management

    func selectLibrary(_ library: LibraryInfo) {
        currentLibrary = library
        currentLibraryId = library.id
        userPreferences.lastSelectedLibraryId = library.id
        saveUserPreferences()

        // Switch library in the daemon connector
        Task {
            await daemonConnector?.switchToLibrary(library)
        }
    }

    // MARK: - Preferences Management

    private func loadUserPreferences() {
        // Load from UserDefaults
        if let data = UserDefaults.standard.data(forKey: "SpacedriveUserPreferences"),
           let preferences = try? JSONDecoder().decode(UserPreferences.self, from: data)
        {
            userPreferences = preferences
            theme = preferences.theme
            sidebarCollapsed = preferences.sidebarCollapsed
        }
    }

    func saveUserPreferences() {
        if let data = try? JSONEncoder().encode(userPreferences) {
            UserDefaults.standard.set(data, forKey: "SpacedriveUserPreferences")
        }
    }

    func updatePreference<T>(_ keyPath: WritableKeyPath<UserPreferences, T>, to value: T) {
        userPreferences[keyPath: keyPath] = value

        // Update published properties if needed
        if keyPath == \.theme {
            theme = userPreferences.theme
        } else if keyPath == \.sidebarCollapsed {
            sidebarCollapsed = userPreferences.sidebarCollapsed
        }

        saveUserPreferences()
    }

    // MARK: - Development Window Configuration

    func setDevWindowConfiguration(_ config: DevWindowConfiguration) {
        updatePreference(\.devWindowConfiguration, to: config)
    }

    func openWindowsForCurrentDevConfiguration() {
        let config = userPreferences.devWindowConfiguration
        print("🔧 Dev configuration set to: \(config.displayName)")

        // Open windows using SwiftUI's window management
        DispatchQueue.main.async {
            switch config {
            case .browserOnly, .development:
                // Open browser window
                if let browserWindow = NSApp.windows.first(where: { $0.title == "Browser" }) {
                    browserWindow.makeKeyAndOrderFront(nil)
                } else {
                    // Create new browser window
                    let window = NSWindow(
                        contentRect: NSRect(x: 0, y: 0, width: 1200, height: 800),
                        styleMask: [.titled, .closable, .miniaturizable, .resizable],
                        backing: .buffered,
                        defer: false
                    )
                    window.title = "Browser"
                    window.contentView = NSHostingView(rootView: BrowserView().withSharedState())
                    window.center()
                    window.makeKeyAndOrderFront(nil)
                }

                if config == .development {
                    // Also open inspector for development mode
                    if let inspectorWindow = NSApp.windows.first(where: { $0.title == "Inspector" }) {
                        inspectorWindow.makeKeyAndOrderFront(nil)
                    } else {
                        let window = NSWindow(
                            contentRect: NSRect(x: 0, y: 0, width: 500, height: 700),
                            styleMask: [.titled, .closable, .miniaturizable, .resizable],
                            backing: .buffered,
                            defer: false
                        )
                        window.title = "Inspector"
                        window.contentView = NSHostingView(rootView: InspectorView().withSharedState())
                        window.center()
                        window.makeKeyAndOrderFront(nil)
                    }
                }

            case .companionOnly, .default:
                // Main companion window is already open
                if let companionWindow = NSApp.windows.first(where: { $0.title == "Spacedrive" }) {
                    companionWindow.makeKeyAndOrderFront(nil)
                }

            case .allWindows:
                // Open all windows
                self.openWindowsForCurrentDevConfiguration() // Recursive call for browser + inspector
                // Add other windows as needed

            case .compact:
                // Compact mode - just companion
                if let companionWindow = NSApp.windows.first(where: { $0.title == "Spacedrive" }) {
                    companionWindow.makeKeyAndOrderFront(nil)
                }
            }
        }
    }

    func getDevWindowConfiguration() -> DevWindowConfiguration {
        return userPreferences.devWindowConfiguration
    }

    func closeAllWindows() {
        NSApp.windows.forEach { $0.close() }
    }

    // MARK: - Action Dispatchers (Redux-like)

    func dispatch(_ action: AppAction) {
        switch action {
        case .connectToDaemon:
            initializeDaemonConnection()

        case .disconnectFromDaemon:
            disconnectDaemon()

        case let .selectLibrary(library):
            selectLibrary(library)

        case let .switchToLibrary(library):
            selectLibrary(library)

        case let .updateTheme(newTheme):
            updatePreference(\.theme, to: newTheme)

        case .toggleSidebar:
            updatePreference(\.sidebarCollapsed, to: !sidebarCollapsed)

        case .refreshJobs:
            daemonConnector?.reconnect()

        case let .pauseJob(jobId):
            daemonConnector?.pauseJob(jobId)

        case let .resumeJob(jobId):
            daemonConnector?.resumeJob(jobId)

        case let .setDevWindowConfiguration(config):
            setDevWindowConfiguration(config)

        case .openDevWindows:
            openWindowsForCurrentDevConfiguration()

        case .closeAllWindows:
            closeAllWindows()
        }
    }

    private func setupConnections() {
        // Auto-connect on app launch
        initializeDaemonConnection()
    }

    // MARK: - SwiftUI Window Management
    // Note: SwiftUI handles window management automatically via WindowGroup
}

// MARK: - Actions (Redux-like action system)

enum AppAction {
    case connectToDaemon
    case disconnectFromDaemon
    case selectLibrary(LibraryInfo)
    case switchToLibrary(LibraryInfo)
    case updateTheme(SpacedriveTheme)
    case toggleSidebar
    case refreshJobs
    case pauseJob(String)
    case resumeJob(String)
    case setDevWindowConfiguration(DevWindowConfiguration)
    case openDevWindows
    case closeAllWindows
}

// MARK: - User Preferences

struct UserPreferences: Codable {
    var theme: SpacedriveTheme = .dark
    var lastSelectedLibraryId: String?
    var sidebarCollapsed: Bool = false
    var windowPositions: [String: WindowPosition] = [:]
    var autoConnectToDaemon: Bool = true
    var showJobNotifications: Bool = true
    var compactMode: Bool = false

    // Development preferences
    var devWindowConfiguration: DevWindowConfiguration = .default
    var devShowAllWindows: Bool = false
    var devAutoOpenBrowser: Bool = false
}

struct WindowPosition: Codable {
    let x: Double
    let y: Double
    let width: Double
    let height: Double
}

// MARK: - Development Window Configuration

enum DevWindowConfiguration: String, CaseIterable, Codable {
    case `default` = "default"
    case browserOnly = "browser_only"
    case companionOnly = "companion_only"
    case allWindows = "all_windows"
    case compact = "compact"
    case development = "development"

    var displayName: String {
        switch self {
        case .default:
            return "Default (Companion + Browser)"
        case .browserOnly:
            return "Browser Only"
        case .companionOnly:
            return "Companion Only"
        case .allWindows:
            return "All Windows"
        case .compact:
            return "Compact Mode"
        case .development:
            return "Development Mode"
        }
    }

    var description: String {
        switch self {
        case .default:
            return "Opens companion window and browser window"
        case .browserOnly:
            return "Opens only the browser window"
        case .companionOnly:
            return "Opens only the companion window"
        case .allWindows:
            return "Opens all available windows"
        case .compact:
            return "Opens companion with compact settings"
        case .development:
            return "Opens browser + inspector for development"
        }
    }

    var shouldAutoOpen: Bool {
        switch self {
        case .browserOnly, .development:
            return true
        default:
            return false
        }
    }
}

// MARK: - Library Info

struct LibraryInfo: Codable, Identifiable {
    let id: String
    let name: String
    let path: String
    let isDefault: Bool
    let stats: LibraryStatistics?

    init(id: String, name: String, path: String, isDefault: Bool = false, stats: LibraryStatistics? = nil) {
        self.id = id
        self.name = name
        self.path = path
        self.isDefault = isDefault
        self.stats = stats
    }
}

// SpacedriveTheme is already Codable in its declaration

// MARK: - View Modifiers for Shared State

struct WithSharedState<Content: View>: View {
    let content: Content

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    var body: some View {
        content
            .environmentObject(SharedAppState.shared)
            .environment(\.spacedriveTheme, SharedAppState.shared.theme)
    }
}

extension View {
    func withSharedState() -> some View {
        WithSharedState { self }
    }
}
