import SwiftUI

struct TahoeSidebar: View {
    @ObservedObject var browserState: BrowserState

    var body: some View {
        VStack(spacing: 0) {
            // Sidebar header
            HStack {
                Text("Locations")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundColor(SpacedriveColors.Text.tertiary)
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
                        .foregroundColor(SpacedriveColors.Text.tertiary)
                }
                .buttonStyle(PlainButtonStyle())
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)

            if !browserState.sidebarCollapsed {
                // Location buttons
                VStack(spacing: 2) {
                    ForEach(browserState.locations) { location in
                        SidebarLocationButton(
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
        .background(SpacedriveColors.Background.secondary)
        .clipShape(UnevenRoundedRectangle(
            topLeadingRadius: 0,
            bottomLeadingRadius: 0,
            bottomTrailingRadius: 13, // 8% increase from 12
            topTrailingRadius: 13
        ))
        .shadow(color: Color.black.opacity(0.1), radius: 8, x: 2, y: 0)
    }
}

#Preview {
    TahoeSidebar(browserState: BrowserState())
        .frame(width: 200, height: 600)
}
