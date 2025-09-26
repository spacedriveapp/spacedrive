import SwiftUI
import UniformTypeIdentifiers

struct ContentArea: View {
    @ObservedObject var browserState: BrowserState

    var body: some View {
        VStack(spacing: 0) {
            // Content header
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text(browserState.selectedLocation?.name ?? "Select a location")
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundColor(SpacedriveColors.Text.primary)

                    Text(browserState.currentPath)
                        .font(.system(size: 12))
                        .foregroundColor(SpacedriveColors.Text.tertiary)
                }

                Spacer()

                // Content actions
                HStack(spacing: 8) {
                    Button(action: {}) {
                        Image(systemName: "list.bullet")
                            .foregroundColor(SpacedriveColors.Text.secondary)
                    }
                    .buttonStyle(PlainButtonStyle())

                    Button(action: {}) {
                        Image(systemName: "grid")
                            .foregroundColor(SpacedriveColors.Text.secondary)
                    }
                    .buttonStyle(PlainButtonStyle())

                    Button(action: {}) {
                        Image(systemName: "slider.horizontal.3")
                            .foregroundColor(SpacedriveColors.Text.secondary)
                    }
                    .buttonStyle(PlainButtonStyle())

                    // Liquid Glass Button
                    LiquidGlassButton(
                        action: {
                            print("Liquid Glass button tapped!")
                        },
                        icon: "sparkles",
                        title: "Glass"
                    )
                }
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 16)

            Divider()
                .background(SpacedriveColors.Border.primary)

            // Main content area
            if browserState.selectedLocation != nil {
                ScrollView {
                    LazyVGrid(columns: Array(repeating: GridItem(.adaptive(minimum: 120), spacing: 16), count: 4), spacing: 16) {
                        ForEach(0..<20, id: \.self) { index in
                            ContentItemView(
                                name: "Item \(index + 1)",
                                isDirectory: index % 3 == 0
                            )
                        }
                    }
                    .padding(20)
                }
            } else {
                // Empty state
                VStack(spacing: 16) {
                    Image(systemName: "folder")
                        .font(.system(size: 48))
                        .foregroundColor(SpacedriveColors.Text.tertiary)

                    Text("Select a location to browse")
                        .font(.system(size: 16, weight: .medium))
                        .foregroundColor(SpacedriveColors.Text.secondary)

                    Text("Choose a location from the sidebar to view its contents")
                        .font(.system(size: 14))
                        .foregroundColor(SpacedriveColors.Text.tertiary)
                        .multilineTextAlignment(.center)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }

            Spacer()
        }
        .background(SpacedriveColors.Background.primary)
        .clipShape(UnevenRoundedRectangle(
            topLeadingRadius: 13, // 8% increase from 12
            bottomLeadingRadius: 13,
            bottomTrailingRadius: 0,
            topTrailingRadius: 0
        ))
        .onDrop(of: [.fileURL], isTargeted: nil) { providers in
            handleFileDrop(providers)
        }
    }

    private func handleFileDrop(_ providers: [NSItemProvider]) -> Bool {
        guard let provider = providers.first else { return false }

        provider.loadItem(forTypeIdentifier: "public.file-url", options: nil) { item, error in
            guard let data = item as? Data,
                  let url = URL(dataRepresentation: data, relativeTo: nil) else {
                return
            }

            let path = url.path
            let fileExtension = url.pathExtension.lowercased()

            // Check if it's an image file
            let imageExtensions = ["jpg", "jpeg", "png", "gif", "bmp", "tiff", "heic", "webp"]
            if imageExtensions.contains(fileExtension) {
                DispatchQueue.main.async {
                    browserState.showImagePreview(path: path)
                }
            }
        }

        return true
    }
}

struct ContentItemView: View {
    let name: String
    let isDirectory: Bool

    var body: some View {
        VStack(spacing: 8) {
            Image(systemName: isDirectory ? "folder.fill" : "doc.fill")
                .font(.system(size: 32))
                .foregroundColor(isDirectory ? SpacedriveColors.Accent.primary : SpacedriveColors.Text.secondary)

            Text(name)
                .font(.system(size: 12, weight: .medium))
                .foregroundColor(SpacedriveColors.Text.primary)
                .lineLimit(2)
                .multilineTextAlignment(.center)
        }
        .frame(width: 80, height: 80)
        .background(SpacedriveColors.Background.secondary)
        .clipShape(UnevenRoundedRectangle(
            topLeadingRadius: 9, // 8% increase from 8
            bottomLeadingRadius: 9,
            bottomTrailingRadius: 9,
            topTrailingRadius: 9
        ))
        .onTapGesture {
            print("Tapped \(name)")
        }
    }
}

