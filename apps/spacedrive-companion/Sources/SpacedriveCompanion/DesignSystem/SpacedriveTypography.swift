import SwiftUI

/// Spacedrive Design System - Typography
/// Centralized font definitions for consistent text styling
struct SpacedriveTypography {

    // MARK: - Font Weights
    enum Weight {
        case thin
        case light
        case regular
        case medium
        case semibold
        case bold
        case heavy
        case black

        var systemWeight: Font.Weight {
            switch self {
            case .thin: return .thin
            case .light: return .light
            case .regular: return .regular
            case .medium: return .medium
            case .semibold: return .semibold
            case .bold: return .bold
            case .heavy: return .heavy
            case .black: return .black
            }
        }
    }

    // MARK: - Typography Scale
    struct Scale {
        // Headlines
        static func h1(_ weight: Weight = .bold) -> Font {
            return .system(size: 32, weight: weight.systemWeight, design: .default)
        }

        static func h2(_ weight: Weight = .bold) -> Font {
            return .system(size: 24, weight: weight.systemWeight, design: .default)
        }

        static func h3(_ weight: Weight = .semibold) -> Font {
            return .system(size: 20, weight: weight.systemWeight, design: .default)
        }

        static func h4(_ weight: Weight = .semibold) -> Font {
            return .system(size: 18, weight: weight.systemWeight, design: .default)
        }

        static func h5(_ weight: Weight = .medium) -> Font {
            return .system(size: 16, weight: weight.systemWeight, design: .default)
        }

        static func h6(_ weight: Weight = .medium) -> Font {
            return .system(size: 14, weight: weight.systemWeight, design: .default)
        }

        // Body Text
        static func body(_ weight: Weight = .regular) -> Font {
            return .system(size: 14, weight: weight.systemWeight, design: .default)
        }

        static func bodyLarge(_ weight: Weight = .regular) -> Font {
            return .system(size: 16, weight: weight.systemWeight, design: .default)
        }

        static func bodySmall(_ weight: Weight = .regular) -> Font {
            return .system(size: 12, weight: weight.systemWeight, design: .default)
        }

        // Labels
        static func label(_ weight: Weight = .medium) -> Font {
            return .system(size: 12, weight: weight.systemWeight, design: .default)
        }

        static func labelLarge(_ weight: Weight = .medium) -> Font {
            return .system(size: 14, weight: weight.systemWeight, design: .default)
        }

        static func labelSmall(_ weight: Weight = .medium) -> Font {
            return .system(size: 10, weight: weight.systemWeight, design: .default)
        }

        // Caption
        static func caption(_ weight: Weight = .regular) -> Font {
            return .system(size: 11, weight: weight.systemWeight, design: .default)
        }

        static func captionSmall(_ weight: Weight = .regular) -> Font {
            return .system(size: 9, weight: weight.systemWeight, design: .default)
        }

        // Code/Monospace
        static func code(_ weight: Weight = .regular) -> Font {
            return .system(size: 12, weight: weight.systemWeight, design: .monospaced)
        }

        static func codeSmall(_ weight: Weight = .regular) -> Font {
            return .system(size: 10, weight: weight.systemWeight, design: .monospaced)
        }
    }
}

// MARK: - Text Style Modifiers
extension Text {
    // Headlines
    func h1(_ weight: SpacedriveTypography.Weight = .bold, color: Color = SpacedriveColors.Text.primary) -> some View {
        self.font(SpacedriveTypography.Scale.h1(weight))
            .foregroundColor(color)
    }

    func h2(_ weight: SpacedriveTypography.Weight = .bold, color: Color = SpacedriveColors.Text.primary) -> some View {
        self.font(SpacedriveTypography.Scale.h2(weight))
            .foregroundColor(color)
    }

    func h3(_ weight: SpacedriveTypography.Weight = .semibold, color: Color = SpacedriveColors.Text.primary) -> some View {
        self.font(SpacedriveTypography.Scale.h3(weight))
            .foregroundColor(color)
    }

    func h4(_ weight: SpacedriveTypography.Weight = .semibold, color: Color = SpacedriveColors.Text.primary) -> some View {
        self.font(SpacedriveTypography.Scale.h4(weight))
            .foregroundColor(color)
    }

    func h5(_ weight: SpacedriveTypography.Weight = .medium, color: Color = SpacedriveColors.Text.primary) -> some View {
        self.font(SpacedriveTypography.Scale.h5(weight))
            .foregroundColor(color)
    }

    func h6(_ weight: SpacedriveTypography.Weight = .medium, color: Color = SpacedriveColors.Text.primary) -> some View {
        self.font(SpacedriveTypography.Scale.h6(weight))
            .foregroundColor(color)
    }

    // Body
    func body(_ weight: SpacedriveTypography.Weight = .regular, color: Color = SpacedriveColors.Text.primary) -> some View {
        self.font(SpacedriveTypography.Scale.body(weight))
            .foregroundColor(color)
    }

    func bodyLarge(_ weight: SpacedriveTypography.Weight = .regular, color: Color = SpacedriveColors.Text.primary) -> some View {
        self.font(SpacedriveTypography.Scale.bodyLarge(weight))
            .foregroundColor(color)
    }

    func bodySmall(_ weight: SpacedriveTypography.Weight = .regular, color: Color = SpacedriveColors.Text.secondary) -> some View {
        self.font(SpacedriveTypography.Scale.bodySmall(weight))
            .foregroundColor(color)
    }

    // Labels
    func label(_ weight: SpacedriveTypography.Weight = .medium, color: Color = SpacedriveColors.Text.secondary) -> some View {
        self.font(SpacedriveTypography.Scale.label(weight))
            .foregroundColor(color)
    }

    func labelLarge(_ weight: SpacedriveTypography.Weight = .medium, color: Color = SpacedriveColors.Text.secondary) -> some View {
        self.font(SpacedriveTypography.Scale.labelLarge(weight))
            .foregroundColor(color)
    }

    func labelSmall(_ weight: SpacedriveTypography.Weight = .medium, color: Color = SpacedriveColors.Text.tertiary) -> some View {
        self.font(SpacedriveTypography.Scale.labelSmall(weight))
            .foregroundColor(color)
    }

    // Caption
    func caption(_ weight: SpacedriveTypography.Weight = .regular, color: Color = SpacedriveColors.Text.tertiary) -> some View {
        self.font(SpacedriveTypography.Scale.caption(weight))
            .foregroundColor(color)
    }

    // Code
    func code(_ weight: SpacedriveTypography.Weight = .regular, color: Color = SpacedriveColors.Text.primary) -> some View {
        self.font(SpacedriveTypography.Scale.code(weight))
            .foregroundColor(color)
    }
}
