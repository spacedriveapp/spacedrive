import SwiftUI
import AppKit

/// Modifier to configure native macOS traffic lights for SwiftUI windows
struct NativeTrafficLightsModifier: ViewModifier {
    func body(content: Content) -> some View {
        content
            .background(NativeTrafficLightsView())
    }
}

/// Background view that configures the window for native traffic lights
struct NativeTrafficLightsView: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView {
        let view = NSView()

        // Configure the window when it becomes available
        DispatchQueue.main.async {
            if let window = view.window {
                configureWindow(window)
            }
        }

        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        // Update if needed
    }

    private func configureWindow(_ window: NSWindow) {
        // Configure for seamless native traffic light integration
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.isMovableByWindowBackground = true

        // Customize title bar appearance
        customizeTitleBarAppearance(window)

        // Ensure proper window behavior
        window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
    }

    private func customizeTitleBarAppearance(_ window: NSWindow) {
        // Set title bar background color to match your app
        DispatchQueue.main.async {
            // Find the title bar view and customize it
            if let titlebarView = window.standardWindowButton(.closeButton)?.superview {
                titlebarView.wantsLayer = true
                titlebarView.layer?.backgroundColor = SpacedriveColors.NSColors.backgroundPrimary.cgColor
            }

            // You can also set the window's background color
            window.backgroundColor = SpacedriveColors.NSColors.backgroundPrimary
        }
    }
}

/// Advanced title bar modifier that allows custom components
struct CustomTitleBarModifier: ViewModifier {
    let centerContent: AnyView?
    let rightContent: AnyView?

    init<CenterContent: View, RightContent: View>(
        center: CenterContent? = nil,
        right: RightContent? = nil
    ) {
        self.centerContent = center.map { AnyView($0) }
        self.rightContent = right.map { AnyView($0) }
    }

    func body(content: Content) -> some View {
        content
            .background(CustomTitleBarView(centerContent: centerContent, rightContent: rightContent))
    }
}

struct CustomTitleBarView: NSViewRepresentable {
    let centerContent: AnyView?
    let rightContent: AnyView?

    func makeNSView(context: Context) -> NSView {
        let view = NSView()

        DispatchQueue.main.async {
            if let window = view.window {
                configureWindow(window)
                addCustomComponents(to: window)
            }
        }

        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        // Update if needed
    }

    private func configureWindow(_ window: NSWindow) {
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.isMovableByWindowBackground = true

        // Customize appearance
        if let titlebarView = window.standardWindowButton(.closeButton)?.superview {
            titlebarView.wantsLayer = true
            titlebarView.layer?.backgroundColor = SpacedriveColors.NSColors.backgroundPrimary.cgColor
        }

        window.backgroundColor = SpacedriveColors.NSColors.backgroundPrimary
        window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
    }

    private func addCustomComponents(to window: NSWindow) {
        guard let titlebarView = window.standardWindowButton(.closeButton)?.superview else { return }

        // Add center content
        if let centerContent = centerContent {
            let hostingView = NSHostingView(rootView: centerContent)
            titlebarView.addSubview(hostingView)

            // Center the content
            hostingView.translatesAutoresizingMaskIntoConstraints = false
            NSLayoutConstraint.activate([
                hostingView.centerXAnchor.constraint(equalTo: titlebarView.centerXAnchor),
                hostingView.centerYAnchor.constraint(equalTo: titlebarView.centerYAnchor)
            ])
        }

        // Add right content
        if let rightContent = rightContent {
            let hostingView = NSHostingView(rootView: rightContent)
            titlebarView.addSubview(hostingView)

            // Position on the right
            hostingView.translatesAutoresizingMaskIntoConstraints = false
            NSLayoutConstraint.activate([
                hostingView.trailingAnchor.constraint(equalTo: titlebarView.trailingAnchor, constant: -12),
                hostingView.centerYAnchor.constraint(equalTo: titlebarView.centerYAnchor)
            ])
        }
    }
}

extension View {
    /// Apply native traffic lights configuration to a SwiftUI view
    func nativeTrafficLights() -> some View {
        self.modifier(NativeTrafficLightsModifier())
    }

    /// Apply custom title bar with optional center and right components
    func customTitleBar<CenterContent: View, RightContent: View>(
        center: CenterContent? = nil,
        right: RightContent? = nil
    ) -> some View {
        self.modifier(CustomTitleBarModifier(center: center, right: right))
    }
}
