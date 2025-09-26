import SwiftUI

/// Settings Navigation Sidebar - Left navigation for settings sections
struct SettingsNavigationView: View {
    @Binding var selectedSection: SettingsSection

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Header
            VStack(alignment: .leading, spacing: 6) {
                Text("Settings")
                    .h3(.semibold)
                    .padding(.horizontal, 20)
                    .padding(.top, 20)

                Text("Configure Spacedrive")
                    .bodySmall(color: SpacedriveColors.Text.secondary)
                    .padding(.horizontal, 20)
                    .padding(.bottom, 20)
            }

            Rectangle()
                .fill(SpacedriveColors.Border.secondary)
                .frame(height: 1)

            // Navigation List
            ScrollView {
                LazyVStack(spacing: 2) {
                    ForEach(SettingsSection.allCases) { section in
                        SettingsNavigationItem(
                            section: section,
                            isSelected: selectedSection == section
                        ) {
                            selectedSection = section
                        }
                    }
                }
                .padding(.vertical, 12)
            }

            Spacer()

            // Footer
            Rectangle()
                .fill(SpacedriveColors.Border.secondary)
                .frame(height: 1)

            VStack(alignment: .leading, spacing: 4) {
                HStack {
                    Circle()
                        .fill(connectionStatusColor)
                        .frame(width: 8, height: 8)

                    Text("Daemon: \(connectionStatusText)")
                        .caption(color: SpacedriveColors.Text.secondary)

                    Spacer()
                }
                .padding(.horizontal, 20)
                .padding(.top, 16)
                .padding(.bottom, 20)
            }
        }
        .background(SpacedriveColors.Background.secondary)
    }

    @EnvironmentObject var appState: SharedAppState

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

    private var connectionStatusText: String {
        switch appState.connectionStatus {
        case .connected:
            return "Connected"
        case .connecting:
            return "Connecting..."
        case .disconnected:
            return "Disconnected"
        case .error:
            return "Error"
        }
    }
}

/// Individual navigation item in the settings sidebar
struct SettingsNavigationItem: View {
    let section: SettingsSection
    let isSelected: Bool
    let action: () -> Void

    @State private var isHovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 12) {
                // Icon with consistent sizing
                Image(systemName: section.icon)
                    .font(.system(size: 16, weight: .medium))
                    .frame(width: 20, height: 20)
                    .foregroundColor(iconColor)

                // Title text
                Text(section.rawValue)
                    .label(.medium, color: textColor)
                    .frame(maxWidth: .infinity, alignment: .leading)

                // Selection indicator
                if isSelected {
                    Circle()
                        .fill(SpacedriveColors.Accent.primary)
                        .frame(width: 6, height: 6)
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            .background(
                RoundedRectangle(cornerRadius: 10)
                    .fill(backgroundColor)
                    .overlay(
                        RoundedRectangle(cornerRadius: 10)
                            .stroke(borderColor, lineWidth: borderWidth)
                    )
            )
            .scaleEffect(isHovered ? 1.02 : 1.0)
            .animation(.easeInOut(duration: 0.2), value: isHovered)
            .animation(.easeInOut(duration: 0.2), value: isSelected)
        }
        .buttonStyle(PlainButtonStyle())
        .padding(.horizontal, 12)
        .onHover { hovering in
            isHovered = hovering
        }
    }

    private var backgroundColor: Color {
        if isSelected {
            return SpacedriveColors.Accent.primary.opacity(0.12)
        } else if isHovered {
            return SpacedriveColors.Interactive.hover.opacity(0.8)
        } else {
            return Color.clear
        }
    }

    private var borderColor: Color {
        if isSelected {
            return SpacedriveColors.Accent.primary.opacity(0.3)
        } else if isHovered {
            return SpacedriveColors.Border.primary.opacity(0.5)
        } else {
            return Color.clear
        }
    }

    private var borderWidth: CGFloat {
        if isSelected || isHovered {
            return 1.0
        } else {
            return 0.0
        }
    }

    private var textColor: Color {
        if isSelected {
            return SpacedriveColors.Accent.primary
        } else {
            return SpacedriveColors.Text.primary
        }
    }

    private var iconColor: Color {
        if isSelected {
            return SpacedriveColors.Accent.primary
        } else {
            return SpacedriveColors.Text.secondary
        }
    }
}

#Preview {
    SettingsNavigationView(selectedSection: .constant(.general))
        .environmentObject(SharedAppState.shared)
        .frame(width: 250, height: 500)
}
