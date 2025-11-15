# Spacedrive Daemon Setup Guide

## Overview

The Spacedrive daemon is a background process that handles file indexing, networking, and core operations. On macOS, running background processes requires special permissions and configuration.

## Production Build Configuration

### 1. macOS Entitlements

The app requires entitlements to spawn daemon processes. These are configured in `src-tauri/Entitlements.plist`.

**To enable in `tauri.conf.json`:**

```json
{
  "bundle": {
    "macOS": {
      "minimumSystemVersion": "10.15",
      "entitlements": "Entitlements.plist"
    }
  }
}
```

### 2. Background Item Permission

macOS 13+ (Ventura) requires explicit user permission for background items.

**User Action Required:**
1. System Settings → General → Login Items & Extensions
2. Find "Spacedrive" in the list
3. Enable the background item permission

The DaemonManager screen includes a button to open these settings directly.

### 3. Daemon Binary Location

The daemon binary (`sd-daemon`) must be bundled with the app:

**Development:**
- Located at: `workspace/target/debug/sd-daemon`
- Built with: `cargo build --bin sd-daemon`

**Production:**
- Must be in app bundle's Resources directory
- Configure in `tauri.conf.json`:

```json
{
  "bundle": {
    "resources": [
      "../../../target/release/sd-daemon"
    ],
    "externalBin": [
      "sd-daemon"
    ]
  }
}
```

## Daemon Operation Modes

### Mode 1: Background Process (Preferred)

Daemon runs as a separate system process:

**Advantages:**
- Survives app restarts
- Lower memory footprint
- Can run headless

**Requirements:**
- macOS background item permission
- `sd-daemon` binary bundled with app

### Mode 2: In-Process (Fallback)

Daemon runs within the Tauri app process:

**Advantages:**
- No permission required
- Always works

**Disadvantages:**
- Daemon dies when app closes
- Higher memory usage

**Implementation Note:** In-process mode is not yet implemented. When permission is denied, the app should fall back to this mode automatically.

## DaemonManager UI

Access via `/daemon` route in the app.

**Features:**
- Real-time daemon status
- Start/Stop controls
- Socket and server URL display
- Settings toggles (auto-start, run in-process)
- Quick link to macOS system settings

## Development

### Testing Daemon Spawn

```bash
# Build daemon
cargo build --bin sd-daemon

# Build and run Tauri app
cd apps/tauri
bun run tauri dev
```

### Check Daemon Status

```bash
# Check if socket exists
ls -la ~/.local/share/spacedrive/daemon/daemon.sock

# Try connecting
echo '{"Query":{"type":"ping"}}' | nc -U ~/.local/share/spacedrive/daemon/daemon.sock
```

### Common Issues

**"Daemon binary not found"**
- Run `cargo build --bin sd-daemon` first
- Check `target/debug/sd-daemon` exists

**"Permission denied" on spawn**
- macOS blocked the background process
- Check System Settings → Login Items
- Enable Spacedrive background permission

**"Socket not created after 3 seconds"**
- Daemon crashed on startup
- Check daemon logs: `~/.local/share/spacedrive/daemon.log`
- Verify data directory permissions

**"Failed to connect to daemon"**
- Socket file exists but daemon not running (stale socket)
- App will auto-clean stale sockets
- Try manual cleanup: `rm ~/.local/share/spacedrive/daemon/daemon.sock`

## Architecture

### Tauri Commands

- `get_daemon_status()` - Get current daemon state
- `start_daemon_process()` - Spawn daemon as background process
- `stop_daemon_process()` - Kill daemon (only if we started it)
- `open_macos_settings()` - Open system settings for permissions
- `get_daemon_socket()` - Get socket path for client connection
- `get_server_url()` - Get HTTP server URL for sidecars

### State Management

```rust
struct DaemonState {
    started_by_us: bool,           // Did we spawn it?
    socket_path: PathBuf,          // Unix socket location
    data_dir: PathBuf,             // Spacedrive data directory
    server_url: Option<String>,    // HTTP server for files
    daemon_process: Option<Child>, // Process handle (if we spawned it)
}
```

### Startup Flow

1. App launches
2. Check if daemon already running (try connect to socket)
3. If running: connect and use it
4. If not running: spawn new daemon process
5. Wait for socket to be created (max 3 seconds)
6. Connect and initialize

### Shutdown Flow

1. App closing
2. Check `started_by_us` flag
3. If true: kill daemon process
4. If false: leave it running (another app instance may be using it)

## Future Enhancements

### TODO: In-Process Mode

Implement fallback when background permission denied:

```rust
#[tauri::command]
async fn start_daemon_in_process(
    data_dir: PathBuf,
    state: State<DaemonState>,
) -> Result<(), String> {
    // Spawn daemon as tokio task instead of separate process
    tauri::async_runtime::spawn(async move {
        sd_daemon::run(data_dir).await
    });
    Ok(())
}
```

### TODO: Settings Persistence

Save user preferences:
- Auto-start daemon on launch
- Prefer in-process mode
- Daemon log level

Use Tauri's store plugin or save to daemon config file.

### TODO: ServiceManagement Framework

For better macOS integration, use Apple's ServiceManagement framework to register the daemon as a login item programmatically.

```rust
// Use cocoa/objc to call SMAppService APIs
// This would show the system permission dialog automatically
```

## Resources

- [macOS Background Task API](https://developer.apple.com/documentation/servicemanagement)
- [Tauri Bundling Guide](https://tauri.app/v1/guides/building/macos)
- [Unix Domain Sockets](https://man7.org/linux/man-pages/man7/unix.7.html)
