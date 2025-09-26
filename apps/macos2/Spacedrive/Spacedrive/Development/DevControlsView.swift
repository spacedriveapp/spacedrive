import SwiftUI

/// Development Controls View
/// A compact view that can be added to any window for quick development window switching
struct DevControlsView: View {
    @StateObject private var devManager = DevWindowManager.shared
    @State private var showingQuickActions = false

    var body: some View {
        VStack(spacing: 8) {
            // Compact toggle button
            Button(action: { showingQuickActions.toggle() }) {
                HStack(spacing: 6) {
                    Image(systemName: "rectangle.3.group")
                        .font(.caption)
                    Text("Dev")
                        .font(.caption)
                        .fontWeight(.medium)
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .background(Color.blue.opacity(0.1))
                .foregroundColor(.blue)
                .cornerRadius(6)
            }
            .buttonStyle(PlainButtonStyle())

            // Quick actions popup
            if showingQuickActions {
                VStack(spacing: 4) {
                    ForEach(DevWindowConfiguration.allCases, id: \.self) { config in
                        Button(action: {
                            devManager.switchTo(config)
                            showingQuickActions = false
                        }) {
                            HStack {
                                Text(config.displayName)
                                    .font(.caption)
                                    .foregroundColor(.primary)
                                Spacer()
                                if devManager.currentConfiguration == config {
                                    Image(systemName: "checkmark")
                                        .font(.caption2)
                                        .foregroundColor(.blue)
                                }
                            }
                            .padding(.horizontal, 8)
                            .padding(.vertical, 4)
                            .background(
                                RoundedRectangle(cornerRadius: 4)
                                    .fill(devManager.currentConfiguration == config ?
                                          Color.blue.opacity(0.1) : Color.clear)
                            )
                        }
                        .buttonStyle(PlainButtonStyle())
                    }
                }
                .padding(8)
                .background(Color(.windowBackgroundColor))
                .cornerRadius(8)
                .shadow(radius: 4)
            }
        }
        .onTapGesture {
            // Close popup when tapping outside
            showingQuickActions = false
        }
    }
}

/// Development Status Bar
/// Shows current configuration and allows quick switching
struct DevStatusBar: View {
    @StateObject private var devManager = DevWindowManager.shared

    var body: some View {
        HStack {
            Image(systemName: "hammer.fill")
                .foregroundColor(.orange)
                .font(.caption)

            Text("Dev:")
                .font(.caption)
                .foregroundColor(.secondary)

            Text(devManager.currentConfiguration.displayName)
                .font(.caption)
                .fontWeight(.medium)
                .foregroundColor(.primary)

            Spacer()

            Button("Switch") {
                // Open the full configuration selector
                NSApp.sendAction(Selector(("showDevConfiguration:")), to: nil, from: nil)
            }
            .font(.caption)
            .buttonStyle(.borderless)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(Color.orange.opacity(0.1))
        .cornerRadius(4)
    }
}

/// Development Window Overlay
/// Can be overlaid on any window to show development controls
struct DevWindowOverlay: View {
    @State private var isVisible = true

    var body: some View {
        if isVisible {
            VStack {
                HStack {
                    Spacer()
                    DevControlsView()
                        .padding(.trailing, 12)
                        .padding(.top, 8)
                }
                Spacer()
            }
            .allowsHitTesting(true)
        }
    }
}

// MARK: - View Extensions for Easy Integration

extension View {
    /// Adds development controls to any view
    func withDevControls() -> some View {
        ZStack {
            self
            DevWindowOverlay()
        }
    }

    /// Adds a development status bar at the top
    func withDevStatusBar() -> some View {
        VStack(spacing: 0) {
            DevStatusBar()
            self
        }
    }
}

// MARK: - Keyboard Shortcuts for Quick Switching

#if DEBUG
struct DevKeyboardShortcuts: View {
    var body: some View {
        EmptyView()
            .onReceive(NotificationCenter.default.publisher(for: NSNotification.Name("devShortcut"))) { notification in
                if let config = notification.object as? DevWindowConfiguration {
                    DevWindowManager.shared.switchTo(config)
                }
            }
    }
}

// Add this to your main view for keyboard shortcuts
// DevKeyboardShortcuts()
#endif
