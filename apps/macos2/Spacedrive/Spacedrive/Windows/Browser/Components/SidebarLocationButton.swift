import SwiftUI

struct SidebarLocationButton: View {
    let location: BrowserLocation
    let isSelected: Bool
    let onTap: () -> Void

    var body: some View {
        Button(action: onTap) {
            HStack(spacing: 8) {
                Image(systemName: location.iconName)
                    .foregroundColor(iconColor)
                    .frame(width: 16, height: 16)

                Text(location.name)
                    .foregroundColor(textColor)
                    .font(.system(size: 13, weight: .medium))

                Spacer()
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .background(backgroundColor)
            .cornerRadius(6)
        }
        .buttonStyle(PlainButtonStyle())
    }

    private var backgroundColor: Color {
        if isSelected {
            return SpacedriveColors.Interactive.selected.opacity(0.3)
        } else {
            return Color.clear
        }
    }

    private var textColor: Color {
        if isSelected {
            return SpacedriveColors.Text.primary
        } else {
            return SpacedriveColors.Text.secondary
        }
    }

    private var iconColor: Color {
        if isSelected {
            return SpacedriveColors.Accent.primary
        } else {
            return SpacedriveColors.Text.tertiary
        }
    }
}


