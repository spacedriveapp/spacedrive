# GPUI Photo Grid

High-performance photo grid viewer for Spacedrive built with GPUI.

## Features

- GPU-accelerated rendering with GPUI
- Grid layout with configurable columns and thumbnail size
- Direct connection to Spacedrive daemon
- Loads up to 1000 media files per directory
- Smart thumbnail variant selection

## Building

```bash
cargo build --release
```

## Running

Make sure Spacedrive daemon is running, then:

```bash
export SD_LIBRARY_ID="your-library-uuid"
export SD_SOCKET_PATH="$HOME/.spacedrive/daemon.sock"  # optional
export SD_HTTP_URL="http://127.0.0.1:54321"            # optional
export SD_INITIAL_PATH="/"                              # optional

cargo run
```

## Helper Script

Use the included script to automatically find your library and run:

```bash
./run.sh
```

## Environment Variables

- `SD_LIBRARY_ID` (required) - Your Spacedrive library UUID
- `SD_SOCKET_PATH` (optional) - Path to daemon socket (default: `~/.spacedrive/daemon.sock`)
- `SD_HTTP_URL` (optional) - HTTP server URL (default: `http://127.0.0.1:54321`)
- `SD_INITIAL_PATH` (optional) - Initial directory path (default: `/`)

## Architecture

- Uses `sd-client` crate to communicate with Spacedrive daemon
- Queries files via Unix socket
- Loads thumbnails via HTTP from sidecar server
- GPUI handles image loading, caching, and GPU rendering

## Performance

Current implementation:
- Loads 1000 files at a time
- Renders all visible thumbnails
- Uses GPUI's built-in image caching

Future optimizations:
- Virtual scrolling (only render visible rows)
- Texture streaming with LRU cache
- Infinite scroll with pagination
- Batch image loading
