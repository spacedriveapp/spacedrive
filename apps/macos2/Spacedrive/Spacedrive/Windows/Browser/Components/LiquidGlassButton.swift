import SwiftUI

/// A button with liquid glass effect styling
struct LiquidGlassButton: View {
    let action: () -> Void
    let icon: String
    let title: String?

    @State private var isPressed = false

    init(action: @escaping () -> Void, icon: String, title: String? = nil) {
        self.action = action
        self.icon = icon
        self.title = title
    }

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                Image(systemName: icon)
                    .font(.system(size: 14, weight: .medium))

                if let title = title {
                    Text(title)
                        .font(.system(size: 12, weight: .medium))
                }
            }
            .foregroundColor(.white)
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background(
                UnevenRoundedRectangle(
                    topLeadingRadius: 9, // 8% increase from 8
                    bottomLeadingRadius: 9,
                    bottomTrailingRadius: 9,
                    topTrailingRadius: 9
                )
                .fill(.ultraThinMaterial)
                .overlay(
                    UnevenRoundedRectangle(
                        topLeadingRadius: 9,
                        bottomLeadingRadius: 9,
                        bottomTrailingRadius: 9,
                        topTrailingRadius: 9
                    )
                    .stroke(.white.opacity(0.2), lineWidth: 1)
                )
            )
            .scaleEffect(isPressed ? 0.95 : 1.0)
            .animation(.easeInOut(duration: 0.1), value: isPressed)
        }
        .buttonStyle(PlainButtonStyle())
        .onLongPressGesture(minimumDuration: 0, maximumDistance: .infinity, pressing: { pressing in
            isPressed = pressing
        }, perform: {})
    }
}

#Preview {
    HStack(spacing: 12) {
        LiquidGlassButton(action: {}, icon: "sparkles", title: "Liquid Glass")
        LiquidGlassButton(action: {}, icon: "wand.and.stars")
        LiquidGlassButton(action: {}, icon: "crystal.ball", title: "Magic")
    }
    .padding()
    .background(Color.black)
}
