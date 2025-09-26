import SpacedriveClient
import SwiftUI

/// Clean macOS-style card showing daemon connectivity and service status
struct ConnectivityCard: View {
    @EnvironmentObject var appState: SharedAppState

    private var coreStatus: CoreStatus? {
        appState.coreStatus
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            // Clean header
            HStack {
                Text("Services")
                    .font(.system(size: 16, weight: .semibold))
                    .foregroundColor(SpacedriveColors.Text.primary)

                Spacer()

                // Simple status indicator
                HStack(spacing: 6) {
                    Circle()
                        .fill(connectionStatusColor)
                        .frame(width: 6, height: 6)

                    Text(appState.connectionStatus.displayName)
                        .font(.system(size: 12))
                        .foregroundColor(SpacedriveColors.Text.secondary)
                }
            }

            // Services in clean rows
            VStack(spacing: 8) {
                ServiceRow(
                    name: "Location Watcher",
                    status: coreStatus?.services.locationWatcher.running == true ? .online : .offline,
                    icon: "doc.text.magnifyingglass"
                )

                ServiceRow(
                    name: "Networking",
                    status: coreStatus?.services.networking.running == true ? .online : .offline,
                    icon: "network"
                )

                ServiceRow(
                    name: "Volume Monitor",
                    status: coreStatus?.services.volumeMonitor.running == true ? .online : .offline,
                    icon: "externaldrive"
                )

                ServiceRow(
                    name: "File Sharing",
                    status: coreStatus?.services.fileSharing.running == true ? .online : .offline,
                    icon: "square.and.arrow.up"
                )
            }

            // Clean info section
            if let status = coreStatus {
                Divider()
                    .background(SpacedriveColors.Border.secondary)
                    .padding(.vertical, 4)

                HStack(spacing: 24) {
                    InfoRow(label: "Libraries", value: "\(status.libraryCount)")
                    InfoRow(label: "Devices", value: "\(status.network.pairedDevices)")
                    InfoRow(label: "Version", value: status.version)
                }
            }
        }
        .padding(16)
        .background(SpacedriveColors.Background.tertiary)
        .cornerRadius(8)
    }

    private var connectionStatusColor: Color {
        switch appState.connectionStatus {
        case .connected:
            return SpacedriveColors.Accent.success
        case .connecting:
            return SpacedriveColors.Accent.warning
        case .disconnected, .error:
            return SpacedriveColors.Accent.error
        }
    }
}

/// Clean macOS-style service row
struct ServiceRow: View {
    let name: String
    let status: ServiceBadgeStatus
    let icon: String

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: icon)
                .font(.system(size: 13))
                .foregroundColor(SpacedriveColors.Text.secondary)
                .frame(width: 16)

            Text(name)
                .font(.system(size: 13))
                .foregroundColor(SpacedriveColors.Text.primary)

            Spacer()

            Circle()
                .fill(statusColor)
                .frame(width: 6, height: 6)
        }
    }

    private var statusColor: Color {
        switch status {
        case .online:
            return SpacedriveColors.Accent.success
        case .offline:
            return SpacedriveColors.Accent.error
        case .degraded:
            return SpacedriveColors.Accent.warning
        }
    }
}

/// Clean macOS-style info row
struct InfoRow: View {
    let label: String
    let value: String

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label)
                .font(.system(size: 11))
                .foregroundColor(SpacedriveColors.Text.secondary)

            Text(value)
                .font(.system(size: 12, weight: .medium))
                .foregroundColor(SpacedriveColors.Text.primary)
        }
    }
}

/// Service status enum
enum ServiceBadgeStatus {
    case online
    case offline
    case degraded

    var displayName: String {
        switch self {
        case .online:
            return "Online"
        case .offline:
            return "Offline"
        case .degraded:
            return "Degraded"
        }
    }
}

#Preview {
    ConnectivityCard()
        .environmentObject(SharedAppState.shared)
        .frame(width: 280)
        .padding()
        .background(SpacedriveColors.Background.primary)
}
