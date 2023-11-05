import AppKit

@objc
public enum AppThemeType: Int {
  case auto = -1
  case light = 0
  case dark = 1
}

@_cdecl("lock_app_theme")
public func lockAppTheme(themeType: AppThemeType) {
  var theme: NSAppearance?
  switch themeType {
  case .auto:
    theme = nil
  case .dark:
    theme = NSAppearance(named: .darkAqua)!
  case .light:
    theme = NSAppearance(named: .aqua)!
  }

  DispatchQueue.main.async {
    NSApp.appearance = theme

    // Trigger a repaint of the window
    if let window = NSApplication.shared.mainWindow {
      window.invalidateShadow()
      window.displayIfNeeded()
    }
  }
}

@_cdecl("blur_window_background")
public func blurWindowBackground(window: NSWindow) {
  let windowContent = window.contentView!
  let blurryView = NSVisualEffectView()

  blurryView.material = .sidebar
  blurryView.state = .followsWindowActiveState
  blurryView.blendingMode = .behindWindow
  blurryView.wantsLayer = true

  window.contentView = blurryView
  blurryView.addSubview(windowContent)
}

@_cdecl("set_titlebar_style")
public func setTitlebarStyle(window: NSWindow, fullScreen: Bool) {
  // this results in far less visual artifacts if we just manage it ourselves (the native taskbar re-appears when fullscreening/un-fullscreening)
  window.titlebarAppearsTransparent = true
  if fullScreen { // fullscreen, give control back to the native OS
    window.toolbar = nil
  } else { // non-fullscreen
    // here we create a uniquely identifiable invisible toolbar in order to correctly pad out the traffic lights
    // this MUST be hidden while fullscreen as macos has a unique dropdown bar for that, and it's far easier to just let it do its thing
    let toolbar = NSToolbar(identifier: "window_invisible_toolbar")
    toolbar.showsBaselineSeparator = false
    window.toolbar = toolbar
  }
  window.titleVisibility = fullScreen ? .visible : .hidden
}
