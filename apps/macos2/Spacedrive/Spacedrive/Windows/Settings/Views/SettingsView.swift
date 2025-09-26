import SwiftUI

/// Settings Window - Main settings interface with left navigation
struct SettingsView: View {
    @EnvironmentObject var appState: SharedAppState
    @State private var selectedSection: SettingsSection = .general

    var body: some View {
        WindowContainer {
            HSplitView {
                // Left Navigation Sidebar
                SettingsNavigationView(selectedSection: $selectedSection)
                    .frame(minWidth: 220, maxWidth: 280)

                // Main Content Area
                SettingsContentView(selectedSection: selectedSection)
                    .frame(minWidth: 450, maxWidth: .infinity)
            }
        }
    }
}

/// Settings sections for navigation
enum SettingsSection: String, CaseIterable, Identifiable {
    case general = "General"
    case daemon = "Daemon"
    case appearance = "Appearance"
    case privacy = "Privacy"
    case advanced = "Advanced"
    case about = "About"

    var id: String { rawValue }

    var icon: String {
        switch self {
        case .general:
            return "gear"
        case .daemon:
            return "server.rack"
        case .appearance:
            return "paintbrush"
        case .privacy:
            return "hand.raised"
        case .advanced:
            return "wrench.and.screwdriver"
        case .about:
            return "info.circle"
        }
    }

    var description: String {
        switch self {
        case .general:
            return "General application settings"
        case .daemon:
            return "Daemon connection and configuration"
        case .appearance:
            return "Theme and visual preferences"
        case .privacy:
            return "Privacy and security settings"
        case .advanced:
            return "Advanced configuration options"
        case .about:
            return "About Spacedrive"
        }
    }
}

#Preview {
    SettingsView()
        .environmentObject(SharedAppState.shared)
        .frame(width: 800, height: 600)
}
