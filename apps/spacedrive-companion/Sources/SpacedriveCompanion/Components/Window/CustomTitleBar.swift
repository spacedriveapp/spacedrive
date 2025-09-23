import SwiftUI
import AppKit

// Environment key for window reference (kept for compatibility)
private struct WindowEnvironmentKey: EnvironmentKey {
    static let defaultValue: NSWindow? = nil
}

extension EnvironmentValues {
    var window: NSWindow? {
        get { self[WindowEnvironmentKey.self] }
        set { self[WindowEnvironmentKey.self] = newValue }
    }
}

struct CustomTitleBar: View {
    @Environment(\.controlActiveState) private var controlActiveState
    @Environment(\.window) private var window
    @State private var isWindowActive = true

    var body: some View {
        HStack {
            // Custom traffic lights
            HStack(spacing: 8) {
                WindowControlButton(type: .close, isActive: isWindowActive)
                WindowControlButton(type: .minimize, isActive: isWindowActive)
                WindowControlButton(type: .zoom, isActive: isWindowActive)
            }
            .padding(.leading, 12)

            Spacer()
        }
        .frame(height: 28)
        .background(Color.clear)
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didBecomeKeyNotification)) { _ in
            updateWindowActiveState()
        }
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didResignKeyNotification)) { _ in
            updateWindowActiveState()
        }
        .onAppear {
            updateWindowActiveState()
        }
    }

    private func updateWindowActiveState() {
        isWindowActive = window?.isKeyWindow ?? false
    }
}

struct WindowControlButton: View {
    enum ButtonType {
        case close, minimize, zoom
    }

    let type: ButtonType
    let isActive: Bool
    @Environment(\.window) private var window
    @State private var isHovered = false

    private var color: Color {
        if !isActive {
            return SpacedriveColors.TrafficLight.inactive
        }

        switch type {
        case .close:
            return SpacedriveColors.TrafficLight.close
        case .minimize:
            return SpacedriveColors.TrafficLight.minimize
        case .zoom:
            return SpacedriveColors.TrafficLight.zoom
        }
    }

    private var systemImage: String {
        switch type {
        case .close:
            return "xmark"
        case .minimize:
            return "minus"
        case .zoom:
            return "plus"
        }
    }

    var body: some View {
        Button(action: performAction) {
            Circle()
                .fill(color)
                .frame(width: 12, height: 12)
                .overlay(
                    // Only show icons when hovered (regardless of window focus)
                    isHovered ?
                    Image(systemName: systemImage)
                        .font(.system(size: 6, weight: .black))
                        .foregroundColor(iconColor)
                    : nil
                )
        }
        .buttonStyle(PlainButtonStyle())
        .help(helpText)
        .onHover { hovering in
            isHovered = hovering
        }
    }

    private var iconColor: Color {
        switch type {
        case .close:
            return SpacedriveColors.TrafficLight.Icons.close
        case .minimize:
            return SpacedriveColors.TrafficLight.Icons.minimize
        case .zoom:
            return SpacedriveColors.TrafficLight.Icons.zoom
        }
    }

    private var helpText: String {
        switch type {
        case .close:
            return "Close"
        case .minimize:
            return "Minimize"
        case .zoom:
            return "Zoom"
        }
    }

    private func performAction() {
        guard let window = window else {
            print("Could not find window for traffic light action")
            return
        }

        switch type {
        case .close:
            window.performClose(nil)
        case .minimize:
            window.performMiniaturize(nil)
        case .zoom:
            window.performZoom(nil)
        }
    }
}
