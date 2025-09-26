import AppKit
import SwiftUI

/// Spacedrive Design System - Color Palette
/// Centralized color definitions for consistent theming across all windows
enum SpacedriveColors {
    // MARK: - Background Colors

    enum Background {
        static let primary = Color(red: 0.07, green: 0.07, blue: 0.09) // Darker main background
        static let secondary = Color(red: 0.12, green: 0.12, blue: 0.14) // Darker box/card backgrounds
        static let tertiary = Color(red: 0.11, green: 0.11, blue: 0.14) // Darker card backgrounds
        static let surface = Color(red: 0.08, green: 0.08, blue: 0.12) // Darker elevated surfaces
    }

    // MARK: - Text Colors

    enum Text {
        static let primary = Color.white
        static let secondary = Color.white.opacity(0.7)
        static let tertiary = Color.white.opacity(0.5)
        static let disabled = Color.white.opacity(0.3)
    }

    // MARK: - Accent Colors

    enum Accent {
        static let primary = Color(red: 0.0, green: 0.48, blue: 1.0) // Spacedrive blue
        static let secondary = Color(red: 0.34, green: 0.34, blue: 0.34) // Neutral
        static let success = Color.green
        static let warning = Color.orange
        static let error = Color.red
        static let info = Color.blue
    }

    // MARK: - Interactive Colors

    enum Interactive {
        static let hover = Color(red: 0.20, green: 0.20, blue: 0.25).opacity(0.5) // Hover effect for #1C1D26 backgrounds
        static let pressed = Color(red: 0.25, green: 0.25, blue: 0.30).opacity(0.6) // Pressed effect
        static let selected = Accent.primary.opacity(0.2)
        static let focus = Accent.primary
    }

    // MARK: - Border Colors

    enum Border {
        static let primary = Color(red: 0.25, green: 0.25, blue: 0.30).opacity(0.3) // Border for #1C1D26 backgrounds
        static let secondary = Color(red: 0.20, green: 0.20, blue: 0.25).opacity(0.2) // Lighter border
        static let focus = Accent.primary
    }

    // MARK: - Traffic Light Colors (Using Native macOS)
    // Native traffic lights are handled by the system - no custom colors needed
}

// MARK: - NSColor Extensions for AppKit Integration

extension SpacedriveColors {
    enum NSColors {
        static let backgroundPrimary = NSColor(red: 0.07, green: 0.07, blue: 0.09, alpha: 1.0) // Darker main background
        static let backgroundSecondary = NSColor(red: 0.08, green: 0.08, blue: 0.12, alpha: 1.0) // Darker box/card backgrounds
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
