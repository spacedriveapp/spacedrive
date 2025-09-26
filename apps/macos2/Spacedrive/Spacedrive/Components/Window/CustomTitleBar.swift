import AppKit
import SwiftUI

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
    var body: some View {
        // Provide spacing for native traffic lights - they will appear in the transparent title bar area
        Spacer()
            .frame(height: 28) // Standard macOS title bar height
    }
}

