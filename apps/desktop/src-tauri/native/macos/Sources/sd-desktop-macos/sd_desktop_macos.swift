import Cocoa

@_cdecl("lock_app_theme")
public func lockAppTheme(themeType: Int) {
    let theme = themeType == 0 ? NSAppearance(named: .aqua) :  NSAppearance(named: .darkAqua);
    NSApp.appearance = theme;
}

@_cdecl("blur_window_background")
public func blurWindowBackground(window: NSWindow) {
    let windowContent = window.contentView!;
    let blurryView = NSVisualEffectView();
    
    blurryView.material = .sidebar;
    blurryView.state = .followsWindowActiveState;
    blurryView.blendingMode = .behindWindow;
    blurryView.wantsLayer = true;

    window.contentView = blurryView;
    blurryView.addSubview(windowContent);
}

@_cdecl("add_invisible_toolbar")
public func addInvisibleToolbar(window: NSWindow, shown: Bool) {
    if !shown {
        window.toolbar = nil;
        return;
    }

    let toolbar = NSToolbar(identifier: "window_invisible_toolbar");

    toolbar.showsBaselineSeparator = false;
    window.toolbar = toolbar;
}

@_cdecl("set_transparent_titlebar")
public func setTransparentTitlebar(window: NSWindow, transparent: Bool, large: Bool) {
    var styleMask = window.styleMask;

    if transparent && large {
        styleMask.insert(.unifiedTitleAndToolbar);
    }

    window.styleMask = styleMask;

    if large {
        addInvisibleToolbar(window: window, shown: true);
    }

    window.titleVisibility = transparent ? .hidden : .visible;
    window.titlebarAppearsTransparent = transparent;
}
