import SwiftUI

/// Spacedrive Button Component - Reusable button with consistent styling
struct SDButton: View {
    enum Style {
        case primary
        case secondary
        case tertiary
        case destructive
        case ghost

        var backgroundColor: Color {
            switch self {
            case .primary:
                return SpacedriveColors.Accent.primary
            case .secondary:
                return SpacedriveColors.Background.surface
            case .tertiary:
                return SpacedriveColors.Background.tertiary
            case .destructive:
                return SpacedriveColors.Accent.error
            case .ghost:
                return Color.clear
            }
        }

        var textColor: Color {
            switch self {
            case .primary:
                return .white
            case .secondary, .tertiary:
                return SpacedriveColors.Text.primary
            case .destructive:
                return .white
            case .ghost:
                return SpacedriveColors.Text.primary
            }
        }

        var borderColor: Color? {
            switch self {
            case .primary, .destructive:
                return nil
            case .secondary, .tertiary:
                return SpacedriveColors.Border.primary
            case .ghost:
                return SpacedriveColors.Border.secondary
            }
        }
    }

    enum Size {
        case small
        case medium
        case large

        var padding: EdgeInsets {
            switch self {
            case .small:
                return EdgeInsets(top: 6, leading: 12, bottom: 6, trailing: 12)
            case .medium:
                return EdgeInsets(top: 8, leading: 16, bottom: 8, trailing: 16)
            case .large:
                return EdgeInsets(top: 12, leading: 20, bottom: 12, trailing: 20)
            }
        }

        var font: Font {
            switch self {
            case .small:
                return SpacedriveTypography.Scale.labelSmall(.medium)
            case .medium:
                return SpacedriveTypography.Scale.label(.medium)
            case .large:
                return SpacedriveTypography.Scale.labelLarge(.medium)
            }
        }

        var cornerRadius: CGFloat {
            switch self {
            case .small:
                return 6
            case .medium:
                return 8
            case .large:
                return 10
            }
        }
    }

    let title: String
    let style: Style
    let size: Size
    let isDisabled: Bool
    let isLoading: Bool
    let icon: String?
    let action: () -> Void

    @State private var isHovered = false
    @State private var isPressed = false

    init(
        _ title: String,
        style: Style = .primary,
        size: Size = .medium,
        isDisabled: Bool = false,
        isLoading: Bool = false,
        icon: String? = nil,
        action: @escaping () -> Void
    ) {
        self.title = title
        self.style = style
        self.size = size
        self.isDisabled = isDisabled
        self.isLoading = isLoading
        self.icon = icon
        self.action = action
    }

    var body: some View {
        Button(action: {
            if !isDisabled && !isLoading {
                action()
            }
        }) {
            HStack(spacing: 6) {
                if isLoading {
                    ProgressView()
                        .progressViewStyle(CircularProgressViewStyle())
                        .scaleEffect(0.8)
                        .frame(width: 12, height: 12)
                } else if let icon = icon {
                    Image(systemName: icon)
                        .font(size.font)
                }

                if !title.isEmpty {
                    Text(title)
                        .font(size.font)
                        .foregroundColor(effectiveTextColor)
                }
            }
            .padding(size.padding)
            .background(effectiveBackgroundColor)
            .cornerRadius(size.cornerRadius)
            .overlay(
                RoundedRectangle(cornerRadius: size.cornerRadius)
                    .stroke(style.borderColor ?? Color.clear, lineWidth: 1)
            )
            .scaleEffect(isPressed ? 0.98 : 1.0)
            .animation(.easeInOut(duration: 0.1), value: isPressed)
            .animation(.easeInOut(duration: 0.2), value: isHovered)
        }
        .buttonStyle(PlainButtonStyle())
        .disabled(isDisabled || isLoading)
        .onHover { hovering in
            isHovered = hovering
        }
        .pressEvents {
            isPressed = true
        } onRelease: {
            isPressed = false
        }
    }

    private var effectiveBackgroundColor: Color {
        if isDisabled {
            return style.backgroundColor.opacity(0.3)
        } else if isPressed {
            return style.backgroundColor.opacity(0.8)
        } else if isHovered {
            return style.backgroundColor.opacity(0.9)
        } else {
            return style.backgroundColor
        }
    }

    private var effectiveTextColor: Color {
        if isDisabled {
            return style.textColor.opacity(0.5)
        } else {
            return style.textColor
        }
    }
}

// MARK: - Press Events Modifier
struct PressEvents: ViewModifier {
    let onPress: () -> Void
    let onRelease: () -> Void

    func body(content: Content) -> some View {
        content
            .simultaneousGesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { _ in
                        onPress()
                    }
                    .onEnded { _ in
                        onRelease()
                    }
            )
    }
}

extension View {
    func pressEvents(onPress: @escaping () -> Void, onRelease: @escaping () -> Void) -> some View {
        modifier(PressEvents(onPress: onPress, onRelease: onRelease))
    }
}

// MARK: - Preview
#Preview {
    VStack(spacing: 16) {
        HStack(spacing: 12) {
            SDButton("Primary", style: .primary, size: .small) {}
            SDButton("Secondary", style: .secondary, size: .small) {}
            SDButton("Tertiary", style: .tertiary, size: .small) {}
        }

        HStack(spacing: 12) {
            SDButton("Medium Primary", style: .primary, size: .medium, icon: "plus") {}
            SDButton("Loading", style: .primary, size: .medium, isLoading: true) {}
            SDButton("Disabled", style: .primary, size: .medium, isDisabled: true) {}
        }

        HStack(spacing: 12) {
            SDButton("Large Ghost", style: .ghost, size: .large, icon: "gear") {}
            SDButton("Destructive", style: .destructive, size: .large) {}
        }
    }
    .padding(20)
    .background(SpacedriveColors.Background.primary)
}
