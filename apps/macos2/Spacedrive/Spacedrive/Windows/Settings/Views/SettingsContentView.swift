import SwiftUI

/// Settings Content Area - Main content that changes based on selected section
struct SettingsContentView: View {
    let selectedSection: SettingsSection

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                // Section Header
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        Image(systemName: selectedSection.icon)
                            .foregroundColor(SpacedriveColors.Accent.primary)
                            .font(.system(size: 22, weight: .medium))

                        VStack(alignment: .leading, spacing: 2) {
                            Text(selectedSection.rawValue)
                                .h2(.semibold)

                            Text(selectedSection.description)
                                .bodySmall(color: SpacedriveColors.Text.secondary)
                        }

                        Spacer()
                    }
                    .padding(.horizontal, 28)
                    .padding(.top, 28)
                    .padding(.bottom, 20)
                }

                Rectangle()
                    .fill(SpacedriveColors.Border.secondary)
                    .frame(height: 1)
                    .padding(.horizontal, 28)

                // Content based on selected section
                contentForSection(selectedSection)
                    .padding(28)
            }
        }
        .background(SpacedriveColors.Background.primary)
    }

    @ViewBuilder
    private func contentForSection(_ section: SettingsSection) -> some View {
        switch section {
        case .general:
            GeneralSettingsView()
        case .daemon:
            DaemonSettingsView()
        case .appearance:
            AppearanceSettingsView()
        case .privacy:
            PrivacySettingsView()
        case .advanced:
            AdvancedSettingsView()
        case .about:
            AboutSettingsView()
        }
    }
}

// MARK: - Individual Settings Views

struct GeneralSettingsView: View {
    @EnvironmentObject var appState: SharedAppState

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            // Services status section
            SettingsGroup("Services") {
                ConnectivityCard()
                    .environmentObject(appState)
            }

            // Auto-launch section
            SettingsGroup("Startup") {
                SettingsToggle(
                    "Launch at login",
                    description: "Automatically start Spacedrive when you log in",
                    isOn: .constant(false)
                )

                SettingsToggle(
                    "Auto-connect to daemon",
                    description: "Automatically connect to the daemon on startup",
                    isOn: .constant(appState.userPreferences.autoConnectToDaemon)
                )
            }

            // Notifications section
            SettingsGroup("Notifications") {
                SettingsToggle(
                    "Show job notifications",
                    description: "Display notifications when jobs complete",
                    isOn: .constant(appState.userPreferences.showJobNotifications)
                )
            }
        }
    }
}

struct DaemonSettingsView: View {
    @EnvironmentObject var appState: SharedAppState

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            SettingsGroup("Connection") {
                HStack {
                    VStack(alignment: .leading) {
                        Text("Status: \(appState.connectionStatus.displayName)")
                            .body(.medium)
                        Text("Manage daemon connection and settings")
                            .bodySmall(color: SpacedriveColors.Text.secondary)
                    }

                    Spacer()

                    SDButton("Connect", style: .primary, size: .medium) {
                        appState.dispatch(.connectToDaemon)
                    }

                    SDButton("Disconnect", style: .secondary, size: .medium) {
                        appState.dispatch(.disconnectFromDaemon)
                    }
                }

                SettingsToggle(
                    "Auto-reconnect",
                    description: "Automatically reconnect if connection is lost",
                    isOn: .constant(true)
                )
            }
        }
    }
}

struct AppearanceSettingsView: View {
    @EnvironmentObject var appState: SharedAppState

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            SettingsGroup("Theme") {
                HStack {
                    Text("Theme:")
                        .body(.medium)

                    Spacer()

                    Picker("Theme", selection: .constant(SpacedriveTheme.dark)) {
                        Text("Dark").tag(SpacedriveTheme.dark)
                        Text("Light").tag(SpacedriveTheme.light)
                    }
                    .pickerStyle(SegmentedPickerStyle())
                    .frame(width: 200)
                }

                SettingsToggle(
                    "Compact mode",
                    description: "Use smaller interface elements",
                    isOn: .constant(appState.userPreferences.compactMode)
                )
            }
        }
    }
}

struct PrivacySettingsView: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            SettingsGroup("Data Collection") {
                SettingsToggle(
                    "Analytics",
                    description: "Help improve Spacedrive by sharing anonymous usage data",
                    isOn: .constant(false)
                )

                SettingsToggle(
                    "Crash reports",
                    description: "Automatically send crash reports to help fix bugs",
                    isOn: .constant(true)
                )
            }
        }
    }
}

struct AdvancedSettingsView: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            SettingsGroup("Debug") {
                HStack {
                    VStack(alignment: .leading) {
                        Text("Developer mode")
                            .body(.medium)
                        Text("Enable advanced debugging features")
                            .bodySmall(color: SpacedriveColors.Text.secondary)
                    }

                    Spacer()

                    SDButton("Enable", style: .secondary, size: .medium) {
                        // Enable developer mode
                    }
                }

                SDButton("Reset all settings", style: .destructive, size: .medium) {
                    // Reset settings
                }
            }
        }
    }
}

struct AboutSettingsView: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            SettingsGroup("Version") {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Spacedrive")
                        .h4(.semibold)
                    Text("Version 0.1.0")
                        .body()
                    Text("Build 1")
                        .bodySmall(color: SpacedriveColors.Text.secondary)
                }
            }

            SettingsGroup("Links") {
                VStack(spacing: 12) {
                    SDButton("Visit Website", style: .secondary, size: .medium, icon: "safari") {
                        // Open website
                    }

                    SDButton("View on GitHub", style: .secondary, size: .medium, icon: "chevron.left.forwardslash.chevron.right") {
                        // Open GitHub
                    }

                    SDButton("Report Issue", style: .secondary, size: .medium, icon: "exclamationmark.bubble") {
                        // Report issue
                    }
                }
            }
        }
    }
}

// MARK: - Settings Components

struct SettingsGroup<Content: View>: View {
    let title: String
    let content: Content

    init(_ title: String, @ViewBuilder content: () -> Content) {
        self.title = title
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(title)
                .h5(.semibold)
                .foregroundColor(SpacedriveColors.Text.primary)

            SDCard(style: .bordered, padding: EdgeInsets(top: 16, leading: 16, bottom: 16, trailing: 16)) {
                content
            }
        }
    }
}

struct SettingsToggle: View {
    let title: String
    let description: String
    @Binding var isOn: Bool

    init(_ title: String, description: String, isOn: Binding<Bool>) {
        self.title = title
        self.description = description
        _isOn = isOn
    }

    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 4) {
                Text(title)
                    .body(.medium)

                Text(description)
                    .bodySmall(color: SpacedriveColors.Text.secondary)
            }

            Spacer()

            Toggle("", isOn: $isOn)
                .toggleStyle(SwitchToggleStyle())
        }
    }
}

#Preview {
    SettingsContentView(selectedSection: .general)
        .environmentObject(SharedAppState.shared)
        .frame(width: 500, height: 600)
}
