import SwiftUI

/// Liquid glass version of the Tahoe sidebar
struct LiquidGlassSidebar: View {
    @ObservedObject var browserState: BrowserState

    var body: some View {
        VStack(spacing: 0) {
            // Sidebar header
            HStack {
                Text("Locations")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundColor(.white.opacity(0.9))
                    .textCase(.uppercase)
                    .tracking(0.5)

                Spacer()

                Button(action: {
                    withAnimation(.easeInOut(duration: 0.2)) {
                        browserState.sidebarCollapsed.toggle()
                    }
                }) {
                    Image(systemName: browserState.sidebarCollapsed ? "chevron.right" : "chevron.left")
                        .font(.system(size: 10, weight: .medium))
                        .foregroundColor(.white.opacity(0.8))
                }
                .buttonStyle(PlainButtonStyle())
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)

            if !browserState.sidebarCollapsed {
                // Location buttons
                VStack(spacing: 2) {
                    ForEach(browserState.locations) { location in
                        LiquidGlassSidebarLocationButton(
                            location: location,
                            isSelected: browserState.selectedLocation?.id == location.id
                        ) {
                            browserState.selectLocation(location)
                        }
                    }
                }
                .padding(.horizontal, 12)
                .padding(.bottom, 12)
            }

            Spacer()
        }
        .frame(width: browserState.sidebarCollapsed ? 60 : 200)
        .background(.ultraThinMaterial)
        .overlay(
            UnevenRoundedRectangle(
                topLeadingRadius: 0,
                bottomLeadingRadius: 0,
                bottomTrailingRadius: 13, // 8% increase from 12
                topTrailingRadius: 13
            )
            .stroke(.white.opacity(0.2), lineWidth: 1)
        )
        .clipShape(UnevenRoundedRectangle(
            topLeadingRadius: 0,
            bottomLeadingRadius: 0,
            bottomTrailingRadius: 13,
            topTrailingRadius: 13
        ))
        .shadow(color: Color.black.opacity(0.1), radius: 8, x: 2, y: 0)
    }
}

/// Liquid glass version of the sidebar location button
struct LiquidGlassSidebarLocationButton: View {
    let location: BrowserLocation
    let isSelected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                Image(systemName: location.iconName)
                    .font(.system(size: 14, weight: .medium))
                    .foregroundColor(.white.opacity(0.9))
                    .frame(width: 16)

                if !isSelected {
                    Text(location.name)
                        .font(.system(size: 13, weight: .medium))
                        .foregroundColor(.white.opacity(0.8))
                        .lineLimit(1)
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background(
                UnevenRoundedRectangle(
                    topLeadingRadius: 9, // 8% increase from 8
                    bottomLeadingRadius: 9,
                    bottomTrailingRadius: 9,
                    topTrailingRadius: 9
                )
                .fill(isSelected ? .white.opacity(0.2) : .clear)
                .overlay(
                    UnevenRoundedRectangle(
                        topLeadingRadius: 9,
                        bottomLeadingRadius: 9,
                        bottomTrailingRadius: 9,
                        topTrailingRadius: 9
                    )
                    .stroke(.white.opacity(0.1), lineWidth: 1)
                )
            )
        }
        .buttonStyle(PlainButtonStyle())
    }
}

#Preview {
    ZStack {
        // Background to show the glass effect
        LinearGradient(
            colors: [.blue, .purple, .pink],
            startPoint: .topLeading,
            endPoint: .bottomTrailing
        )
        .ignoresSafeArea()

        HStack {
            LiquidGlassSidebar(browserState: BrowserState())
            Spacer()
        }
    }
    .frame(width: 1200, height: 800)
}
