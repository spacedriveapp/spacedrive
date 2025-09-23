import AppKit
import SwiftUI

class RoundedWindow: NSWindow {
    override init(contentRect: NSRect, styleMask style: NSWindow.StyleMask, backing backingStoreType: NSWindow.BackingStoreType, defer flag: Bool) {
        super.init(contentRect: contentRect, styleMask: style, backing: backingStoreType, defer: flag)

        setupWindow()
    }

    private func setupWindow() {
        // Configure window properties
        self.isOpaque = false
        self.backgroundColor = NSColor.clear
        self.hasShadow = true
        self.isMovableByWindowBackground = true
        self.minSize = NSSize(width: 300, height: 400)

        // Window behavior (level is set by WindowManager)
        self.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
    }

    override var canBecomeKey: Bool {
        return true
    }

    override var canBecomeMain: Bool {
        return true
    }
}

struct RoundedBackgroundView: NSViewRepresentable {
    let cornerRadius: CGFloat
    let backgroundColor: NSColor

    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        view.wantsLayer = true

        let layer = CALayer()
        layer.backgroundColor = backgroundColor.cgColor
        layer.cornerRadius = cornerRadius
        layer.masksToBounds = true

        view.layer = layer
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        nsView.layer?.backgroundColor = backgroundColor.cgColor
        nsView.layer?.cornerRadius = cornerRadius
    }
}
