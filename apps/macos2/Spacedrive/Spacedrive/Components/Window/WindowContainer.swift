import AppKit
import SwiftUI

/// Shared Window Container - Provides consistent window chrome with native traffic lights for all windows
struct WindowContainer<Content: View>: View {
    let content: Content

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    var body: some View {
        ZStack {
            // Rounded background using design system colors with Tahoe-style corners
            UnevenRoundedRectangle(
                topLeadingRadius: 13, // 8% increase from 12
                bottomLeadingRadius: 13,
                bottomTrailingRadius: 13,
                topTrailingRadius: 13
            )
            .fill(Color(SpacedriveColors.NSColors.backgroundPrimary))

            VStack(spacing: 0) {
                // Spacer for native title bar area (where traffic lights will be)
                Spacer()
                    .frame(height: 28)

                // Main content area
                content
            }
        }
    }
}

#Preview {
    WindowContainer {
        VStack {
            Text("Sample Window Content")
                .h3()
            Text("This is how content appears inside the window container")
                .bodySmall(color: SpacedriveColors.Text.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(20)
    }
    .frame(width: 400, height: 300)
}
