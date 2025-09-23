import SwiftUI

/// Spacedrive Card Component - Reusable container with consistent styling
struct SDCard<Content: View>: View {
    enum Style {
        case elevated
        case bordered
        case flat

        var backgroundColor: Color {
            switch self {
            case .elevated:
                return SpacedriveColors.Background.surface
            case .bordered:
                return SpacedriveColors.Background.tertiary
            case .flat:
                return SpacedriveColors.Background.secondary
            }
        }

        var borderColor: Color? {
            switch self {
            case .elevated, .flat:
                return nil
            case .bordered:
                return SpacedriveColors.Border.primary
            }
        }

        var shadowRadius: CGFloat {
            switch self {
            case .elevated:
                return 8
            case .bordered, .flat:
                return 0
            }
        }
    }

    let style: Style
    let padding: EdgeInsets
    let cornerRadius: CGFloat
    let content: Content

    @State private var isHovered = false

    init(
        style: Style = .elevated,
        padding: EdgeInsets = EdgeInsets(top: 16, leading: 16, bottom: 16, trailing: 16),
        cornerRadius: CGFloat = 12,
        @ViewBuilder content: () -> Content
    ) {
        self.style = style
        self.padding = padding
        self.cornerRadius = cornerRadius
        self.content = content()
    }

    var body: some View {
        content
            .padding(padding)
            .background(effectiveBackgroundColor)
            .cornerRadius(cornerRadius)
            .overlay(
                RoundedRectangle(cornerRadius: cornerRadius)
                    .stroke(style.borderColor ?? Color.clear, lineWidth: 1)
            )
            .shadow(
                color: Color.black.opacity(0.2),
                radius: style.shadowRadius,
                x: 0,
                y: style.shadowRadius / 2
            )
            .scaleEffect(isHovered ? 1.02 : 1.0)
            .animation(.easeInOut(duration: 0.2), value: isHovered)
            .onHover { hovering in
                if style == .elevated {
                    isHovered = hovering
                }
            }
    }

    private var effectiveBackgroundColor: Color {
        if isHovered && style == .elevated {
            return style.backgroundColor.opacity(0.9)
        } else {
            return style.backgroundColor
        }
    }
}

// MARK: - Specialized Card Types
struct SDJobCard<Content: View>: View {
    let content: Content

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    var body: some View {
        SDCard(
            style: .bordered,
            padding: EdgeInsets(top: 12, leading: 12, bottom: 12, trailing: 12),
            cornerRadius: 8
        ) {
            content
        }
    }
}

struct SDStatusCard<Content: View>: View {
    let content: Content

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    var body: some View {
        SDCard(
            style: .flat,
            padding: EdgeInsets(top: 8, leading: 12, bottom: 8, trailing: 12),
            cornerRadius: 6
        ) {
            content
        }
    }
}

// MARK: - Preview
#Preview {
    VStack(spacing: 16) {
        SDCard(style: .elevated) {
            VStack(alignment: .leading, spacing: 8) {
                Text("Elevated Card")
                    .h5()
                Text("This is an elevated card with shadow and hover effects.")
                    .bodySmall(color: SpacedriveColors.Text.secondary)

                HStack {
                    SDButton("Action", style: .primary, size: .small) {}
                    SDButton("Cancel", style: .secondary, size: .small) {}
                    Spacer()
                }
            }
        }

        SDCard(style: .bordered) {
            HStack {
                VStack(alignment: .leading) {
                    Text("Bordered Card")
                        .h6()
                    Text("With border styling")
                        .caption()
                }
                Spacer()
                Image(systemName: "folder")
                    .foregroundColor(SpacedriveColors.Accent.primary)
            }
        }

        SDJobCard {
            HStack {
                Circle()
                    .fill(SpacedriveColors.Accent.success)
                    .frame(width: 8, height: 8)

                VStack(alignment: .leading) {
                    Text("Processing Files")
                        .label()
                    Text("45% complete")
                        .caption()
                }

                Spacer()
            }
        }
    }
    .padding(20)
    .background(SpacedriveColors.Background.primary)
}
