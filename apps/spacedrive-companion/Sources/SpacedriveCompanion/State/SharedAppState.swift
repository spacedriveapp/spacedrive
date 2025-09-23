import SwiftUI
import Combine

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
    @Published var jobsLastUpdated: Date = Date()

    // MARK: - Library State
    @Published var currentLibrary: LibraryInfo?
    @Published var availableLibraries: [LibraryInfo] = []

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
        userPreferences.lastSelectedLibraryId = library.id
        saveUserPreferences()
    }

    // MARK: - Preferences Management
    private func loadUserPreferences() {
        // Load from UserDefaults
        if let data = UserDefaults.standard.data(forKey: "SpacedriveUserPreferences"),
           let preferences = try? JSONDecoder().decode(UserPreferences.self, from: data) {
            userPreferences = preferences
            theme = preferences.theme
            sidebarCollapsed = preferences.sidebarCollapsed
        }
    }

    private func saveUserPreferences() {
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

    // MARK: - Action Dispatchers (Redux-like)
    func dispatch(_ action: AppAction) {
        switch action {
        case .connectToDaemon:
            initializeDaemonConnection()

        case .disconnectFromDaemon:
            disconnectDaemon()

        case .selectLibrary(let library):
            selectLibrary(library)

        case .updateTheme(let newTheme):
            updatePreference(\.theme, to: newTheme)

        case .toggleSidebar:
            updatePreference(\.sidebarCollapsed, to: !sidebarCollapsed)

        case .refreshJobs:
            daemonConnector?.reconnect()

        case .openWindow(let type, let id):
            WindowManager.shared.showWindow(type: type, id: id)
        }
    }

    private func setupConnections() {
        // Auto-connect on app launch
        initializeDaemonConnection()
    }
}

// MARK: - Actions (Redux-like action system)
enum AppAction {
    case connectToDaemon
    case disconnectFromDaemon
    case selectLibrary(LibraryInfo)
    case updateTheme(SpacedriveTheme)
    case toggleSidebar
    case refreshJobs
    case openWindow(WindowType, String?)
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
}

struct WindowPosition: Codable {
    let x: Double
    let y: Double
    let width: Double
    let height: Double
}

// MARK: - Library Info
struct LibraryInfo: Codable, Identifiable {
    let id: String
    let name: String
    let path: String
    let isDefault: Bool
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
