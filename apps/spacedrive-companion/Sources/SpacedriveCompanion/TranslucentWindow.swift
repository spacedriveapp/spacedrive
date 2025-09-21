import AppKit
import SwiftUI

class TranslucentWindow: NSWindow {
    override init(contentRect: NSRect, styleMask style: NSWindow.StyleMask, backing backingStoreType: NSWindow.BackingStoreType, defer flag: Bool) {
        super.init(contentRect: contentRect, styleMask: style, backing: backingStoreType, defer: flag)

        setupWindow()
    }

    private func setupWindow() {
        // Make the window translucent
        self.isOpaque = false
        self.backgroundColor = NSColor.clear

        // Add visual effect view for blur
        let visualEffectView = NSVisualEffectView()
        visualEffectView.material = .hudWindow
        visualEffectView.blendingMode = .behindWindow
        visualEffectView.state = .active

        // Set the visual effect view as the content view's background
        if let contentView = self.contentView {
            visualEffectView.frame = contentView.bounds
            visualEffectView.autoresizingMask = [.width, .height]
            contentView.addSubview(visualEffectView, positioned: .below, relativeTo: nil)
        }

        // Window behavior
        self.level = .floating
        self.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]

        // Make window movable by background
        self.isMovableByWindowBackground = true

        // Set minimum size
        self.minSize = NSSize(width: 300, height: 400)
    }
}

struct VisualEffectBackground: NSViewRepresentable {
    func makeNSView(context: Context) -> NSVisualEffectView {
        let view = NSVisualEffectView()
        view.material = .hudWindow
        view.blendingMode = .behindWindow
        view.state = .active
        return view
    }

    func updateNSView(_ nsView: NSVisualEffectView, context: Context) {
        // No updates needed
    }
}


