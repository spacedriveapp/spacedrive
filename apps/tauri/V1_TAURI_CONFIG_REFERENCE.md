# V1 Tauri Configuration Reference

This document captures the sophisticated Tauri configuration from V1 that should be incrementally ported to V2.

## Current V2 Status

**Already Ported:**
- Basic window configuration (1400x750, hidden title, transparent)
- `app_ready` command for controlled window showing
- Core plugins: dialog, fs, shell, clipboard-manager, os
- macOS private API enabled
- Basic CSP configuration
- Bundle settings for Linux/macOS/Windows

**TODO - High Priority:**
- [ ] Custom Tauri commands (file operations, menu management, reveal items)
- [ ] Platform-specific code (macOS Swift bridge, Linux/Windows file ops)
- [ ] Menu system (macOS menu bar with keybinds)
- [ ] Drag and drop tracking system
- [ ] Window effects (blur, vibrancy)
- [ ] Deep-link plugin for `spacedrive://` URL scheme

**TODO - Medium Priority:**
- [ ] Updater plugin and custom update flow
- [ ] Custom server plugin (for serving thumbnails)
- [ ] Error plugin (for pre-rspc error display)
- [ ] TypeScript bindings with tauri-specta
- [ ] Full screen detection and titlebar style switching

**TODO - Lower Priority:**
- [ ] CORS fetch plugin (for Supertokens auth)
- [ ] Linux environment normalization (XDG, GStreamer, Flatpak/Snap detection)
- [ ] Windows file association APIs
- [ ] AI models bundling (onnxruntime)

---

## V1 Custom Tauri Commands

These should be ported as needed:

### App Lifecycle
```rust
app_ready                  // Already ported
reset_spacedrive          // TODO: Wipes data directory
reload_webview            // TODO: Platform-specific webview reload
```

### Menu & UI
```rust
set_menu_bar_item_state   // TODO: Enable/disable menu items
refresh_menu_bar          // TODO: Update menu based on library
lock_app_theme            // TODO: Force light/dark mode (macOS)
```

### File Operations
```rust
open_file_paths           // TODO: Open files by library ID
open_ephemeral_files      // TODO: Open files by path
get_file_path_open_with_apps        // TODO: Get "Open With" apps
get_ephemeral_files_open_with_apps  // TODO: Same for paths
open_file_path_with       // TODO: Open with specific app
open_ephemeral_file_with  // TODO: Open path with specific app
reveal_items              // TODO: Reveal in file manager
open_logs_dir             // TODO: Open logs directory
open_trash_in_os_explorer // TODO: Open OS trash
```

### Drag & Drop
```rust
start_drag                // TODO: Cursor-tracked drag (macOS/Windows)
stop_drag                 // TODO: Stop drag tracking
```

### macOS Specific
```rust
request_fda_macos         // TODO: Request Full Disk Access
set_titlebar_style        // TODO: Custom titlebar
disable_app_nap           // TODO: Prevent sleep during indexing
enable_app_nap            // TODO: Re-enable sleep
```

### Updater
```rust
check_for_update          // TODO: Check for updates
install_update            // TODO: Download and install
```

---

## V1 Window Configuration

```json
{
  "width": 1400,
  "height": 750,
  "minWidth": 768,
  "minHeight": 500,
  "hiddenTitle": true,
  "transparent": true,
  "center": true,
  "visible": false,           // Shown via app_ready
  "dragDropEnabled": true,
  "windowEffects": {
    "effects": ["sidebar"],
    "state": "followsWindowActiveState",
    "radius": 9
  }
}
```

**Current V2:** Basic config without windowEffects (add when needed).

---

## V1 Content Security Policy

```json
{
  "default-src": "'self' webkit-pdfjs-viewer: asset: http://asset.localhost blob: data: filesystem: http: https: tauri:",
  "connect-src": "'self' ipc: http://ipc.localhost ws: wss: http: https: tauri:",
  "img-src": "'self' asset: http://asset.localhost blob: data: filesystem: http: https: tauri:",
  "style-src": "'self' 'unsafe-inline' http: https: tauri:"
}
```

**Current V2:** Simplified CSP (expand as we add features like custom server).

---

## V1 Bundle Configuration

### Linux
```json
{
  "deb": {
    "depends": ["libc6", "libxdo3", "dbus", "libwebkit2gtk-4.1-0", "libgtk-3-0"],
    "files": {
      "/usr/share/spacedrive/models/yolov8s.onnx": "../../../models/yolov8s.onnx"
    }
  }
}
```

**Critical:** dbus must NOT be vendored (breaks on X11/Nvidia).

### macOS
```json
{
  "minimumSystemVersion": "10.15",
  "frameworks": [".deps/Spacedrive.framework"]
}
```

**V1 Pattern:** Custom Swift framework bundled (may need for V2 Swift bridge).

### Windows
```json
{
  "webviewInstallMode": {
    "type": "embedBootstrapper",
    "silent": true
  }
}
```

---

## V1 Plugins

### Official Tauri Plugins
```toml
tauri-plugin-clipboard-manager = "2.0"  # Ported
tauri-plugin-deep-link = "2.0"          # TODO
tauri-plugin-dialog = "2.0"             # Ported
tauri-plugin-drag = "2.0"               # TODO (custom fork with move operation)
tauri-plugin-http = "2.0"               # TODO (if needed)
tauri-plugin-os = "2.0"                 # Ported
tauri-plugin-shell = "2.0"              # Ported
tauri-plugin-updater = "2.0"            # TODO
```

### Custom Plugins
- **tauri-plugin-cors-fetch:** Modified for Supertokens auth (TODO)
- **sd_server_plugin:** Axum server for custom URI protocol (TODO)
- **sd_error_plugin:** Injects `window.__SD_ERROR__` (TODO)
- **Updater injection:** Injects `window.__SD_UPDATER__` flag (TODO)

---

## V1 Platform-Specific Code

### macOS (Swift via swift-rs)

**Window Management:**
- `set_titlebar_style` - Invisible toolbar trick
- `disable_app_nap` / `enable_app_nap` - System sleep control
- `lock_app_theme` - Force light/dark mode
- `reload_webview` - Proper reload without artifacts

**File Operations:**
- `get_open_with_applications` - Apps that can open file (with icons)
- `open_file_path_with` - Open with specific application
- Fallback compatibility for macOS 10.15+

### Linux

**Environment Normalization (critical!):**
- XDG directory setup (HOME, DATA_HOME, CONFIG_HOME, etc.)
- GStreamer plugin paths
- PATH normalization
- Flatpak/Snap detection
- NVIDIA GPU workaround: `WEBKIT_DISABLE_DMABUF_RENDERER=1`

**File Operations:**
- GTK-based "Open With" detection
- Content type detection
- AppLaunchContext for opening files
- Fallback `getpwuid_r` for $HOME

### Windows

**File Operations:**
- `list_apps_associated_with_ext` - Uses `SHAssocEnumHandlers`
- `open_file_path_with` - Uses `IAssocHandler` and `IShellItem`
- COM initialization (thread-local with atexit)

---

## V1 Menu System (macOS)

Full macOS menu bar:

```
Spacedrive
  ├─ About Spacedrive
  ├─ New Library         [Cmd+N]
  ├─ Hide                [Cmd+H]
  └─ Quit                [Cmd+Q]

Edit
  ├─ Undo                [Cmd+Z]
  ├─ Redo                [Cmd+Shift+Z]
  └─ Select All          [Cmd+A]

View
  ├─ Overview            [Cmd+Shift+O]
  ├─ Search              [Cmd+F]
  ├─ Settings            [Cmd+,]
  ├─ Layouts
  └─ Dev Tools

Window
  ├─ Minimize            [Cmd+M]
  ├─ Fullscreen          [Cmd+Ctrl+F]
  └─ Reload Webview
```

**Features:**
- Library-locked menu items (disabled until library loaded)
- Keybind forwarding to frontend
- Fullscreen detection with titlebar switching

---

## V1 Drag & Drop Implementation

**Advanced Features:**
- Cursor position tracking at 8ms intervals
- Detects when cursor leaves window bounds
- Creates drag session with base64 image preview
- Callback for drop result and position
- Linux: Disabled (not implemented)

**Custom `drag` crate:** Modified fork with "move-operation" branch from spacedriveapp.

---

## V1 Key Gotchas

1. **Linux dbus:** MUST NOT be vendored (breaks on X11/Nvidia)
2. **GTK version:** Must match Tauri's (0.18 with v3_24 feature)
3. **NVIDIA workaround:** `WEBKIT_DISABLE_DMABUF_RENDERER=1`
4. **Swift NULL delimiter hack:** For file path arrays
5. **Updater:** Disabled on Linux
6. **Tauri version:** Pinned to v2.0.6 exactly
7. **AI models:** Special linker config for onnxruntime TLS

---

## Next Steps for V2

1. **Immediate:** Keep current basic setup, focus on core integration
2. **Phase 2:** Add file operation commands as UI needs them
3. **Phase 3:** Port platform-specific code (Swift bridge, Linux normalization)
4. **Phase 4:** Add menu system and keyboard shortcuts
5. **Phase 5:** Advanced features (updater, drag tracking, window effects)

**Principle:** Port incrementally as features are needed, don't front-load everything.
