import SwiftUI
import AppKit

/// Image preview view with zoom and pan capabilities
struct ImagePreviewView: View {
    let imagePath: String
    let onClose: () -> Void

    @State private var scale: CGFloat = 1.0
    @State private var lastScale: CGFloat = 1.0
    @State private var offset: CGSize = .zero
    @State private var lastOffset: CGSize = .zero
    @State private var image: NSImage?
    @State private var imageSize: CGSize = .zero

    // Sidebar width for initial positioning
    private let sidebarWidth: CGFloat = 200

    var body: some View {
        ZStack {
            // Background
            Color.black
                .ignoresSafeArea()

            if let image = image {
                // Image with zoom and pan
                Image(nsImage: image)
                    .resizable()
                    .aspectRatio(contentMode: .fit)
                    .scaleEffect(scale)
                    .offset(offset)
                    .gesture(
                        SimultaneousGesture(
                            // Pinch to zoom
                            MagnificationGesture()
                                .onChanged { value in
                                    let delta = value / lastScale
                                    lastScale = value
                                    scale *= delta
                                }
                                .onEnded { _ in
                                    lastScale = 1.0
                                    // Constrain scale
                                    scale = max(0.5, min(scale, 5.0))
                                },

                            // Drag to pan
                            DragGesture()
                                .onChanged { value in
                                    offset = CGSize(
                                        width: lastOffset.width + value.translation.width,
                                        height: lastOffset.height + value.translation.height
                                    )
                                }
                                .onEnded { _ in
                                    lastOffset = offset
                                }
                        )
                    )
                    .onTapGesture(count: 2) {
                        // Double tap to reset zoom
                        withAnimation(.easeInOut(duration: 0.3)) {
                            scale = 1.0
                            offset = .zero
                            lastOffset = .zero
                        }
                    }
            } else {
                // Loading state
                VStack(spacing: 16) {
                    ProgressView()
                        .progressViewStyle(CircularProgressViewStyle(tint: .white))
                        .scaleEffect(1.5)

                    Text("Loading image...")
                        .foregroundColor(.white)
                        .font(.system(size: 16))
                }
            }

            // Close button
            VStack {
                HStack {
                    Spacer()
                    Button(action: onClose) {
                        Image(systemName: "xmark.circle.fill")
                            .font(.system(size: 24))
                            .foregroundColor(.white.opacity(0.8))
                            .background(Color.black.opacity(0.3))
                            .clipShape(Circle())
                    }
                    .buttonStyle(PlainButtonStyle())
                    .padding(.top, 20)
                    .padding(.trailing, 20)
                }
                Spacer()
            }
        }
        .onAppear {
            loadImage()
            // Initial positioning to compensate for sidebar
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
                adjustInitialPosition()
            }
        }
    }

    private func loadImage() {
        guard let nsImage = NSImage(contentsOfFile: imagePath) else {
            print("Failed to load image from: \(imagePath)")
            return
        }

        self.image = nsImage
        self.imageSize = nsImage.size
    }

    private func adjustInitialPosition() {
        // Center the image in the available space (accounting for sidebar)
        // This ensures the image appears centered in the content area initially
        let availableWidth = NSScreen.main?.frame.width ?? 1200

        // Calculate the offset needed to center the image in the content area
        // (not the full screen, but the area to the right of the sidebar)
        let contentAreaWidth = availableWidth - sidebarWidth
        let contentAreaCenterX = (contentAreaWidth / 2) + sidebarWidth

        // Center horizontally in the content area
        offset = CGSize(
            width: contentAreaCenterX - (availableWidth / 2),
            height: 0
        )
        lastOffset = offset
    }
}

#Preview {
    ImagePreviewView(
        imagePath: "/System/Library/Desktop Pictures/Monterey.heic",
        onClose: {}
    )
    .frame(width: 1200, height: 800)
}
