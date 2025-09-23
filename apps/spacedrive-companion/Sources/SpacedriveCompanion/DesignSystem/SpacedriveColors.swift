import SwiftUI
import AppKit

/// Spacedrive Design System - Color Palette
/// Centralized color definitions for consistent theming across all windows
struct SpacedriveColors {

    // MARK: - Background Colors
    struct Background {
        static let primary = Color(red: 0.08, green: 0.08, blue: 0.08) // Near black
        static let secondary = Color(red: 0.12, green: 0.12, blue: 0.12) // Slightly lighter
        static let tertiary = Color(red: 0.16, green: 0.16, blue: 0.16) // Card backgrounds
        static let surface = Color(red: 0.20, green: 0.20, blue: 0.20) // Elevated surfaces
    }

    // MARK: - Text Colors
    struct Text {
        static let primary = Color.white
        static let secondary = Color.white.opacity(0.7)
        static let tertiary = Color.white.opacity(0.5)
        static let disabled = Color.white.opacity(0.3)
    }

    // MARK: - Accent Colors
    struct Accent {
        static let primary = Color(red: 0.0, green: 0.48, blue: 1.0) // Spacedrive blue
        static let secondary = Color(red: 0.34, green: 0.34, blue: 0.34) // Neutral
        static let success = Color.green
        static let warning = Color.orange
        static let error = Color.red
        static let info = Color.blue
    }

    // MARK: - Interactive Colors
    struct Interactive {
        static let hover = Color.white.opacity(0.1)
        static let pressed = Color.white.opacity(0.2)
        static let selected = Accent.primary.opacity(0.2)
        static let focus = Accent.primary
    }

    // MARK: - Border Colors
    struct Border {
        static let primary = Color.white.opacity(0.1)
        static let secondary = Color.white.opacity(0.05)
        static let focus = Accent.primary
    }

    // MARK: - Traffic Light Colors (Native macOS)
    struct TrafficLight {
        static let close = Color(red: 0.98, green: 0.37, blue: 0.37)
        static let minimize = Color(red: 1.0, green: 0.74, blue: 0.18)
        static let zoom = Color(red: 0.15, green: 0.78, blue: 0.15)
        static let inactive = Color.gray.opacity(0.3)

        struct Icons {
            static let close = Color(red: 0.48, green: 0.1, blue: 0.1)
            static let minimize = Color(red: 0.64, green: 0.44, blue: 0.0)
            static let zoom = Color(red: 0.0, green: 0.48, blue: 0.0)
        }
    }
}

// MARK: - NSColor Extensions for AppKit Integration
extension SpacedriveColors {
    struct NSColors {
        static let backgroundPrimary = NSColor(red: 0.08, green: 0.08, blue: 0.08, alpha: 1.0)
        static let backgroundSecondary = NSColor(red: 0.12, green: 0.12, blue: 0.12, alpha: 1.0)
        static let accentPrimary = NSColor(red: 0.0, green: 0.48, blue: 1.0, alpha: 1.0)
    }
}

// MARK: - Environment Key for Theme
private struct ThemeEnvironmentKey: EnvironmentKey {
    static let defaultValue: SpacedriveTheme = .dark
}

extension EnvironmentValues {
    var spacedriveTheme: SpacedriveTheme {
        get { self[ThemeEnvironmentKey.self] }
        set { self[ThemeEnvironmentKey.self] = newValue }
    }
}

enum SpacedriveTheme: Codable {
    case dark
    case light // Future support
}
