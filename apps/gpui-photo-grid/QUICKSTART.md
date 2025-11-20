# Quick Start Guide

## What You Just Built

A standalone GPUI application that:
- Connects to your running Spacedrive daemon
- Queries media files (images/videos)
- Renders them in a GPU-accelerated grid
- Loads thumbnails directly from Spacedrive's HTTP server

## Prerequisites

1. **Spacedrive must be running** - The daemon needs to be active
2. **Know your library ID** - The run script will find it automatically

## Running It

### Option 1: Automatic (Recommended)

```bash
cd apps/gpui-photo-grid
./run.sh
```

This will:
- Find your Spacedrive library automatically
- Set all environment variables
- Run the app in release mode

### Option 2: Manual

```bash
# Find your library ID
cat "$HOME/Library/Application Support/spacedrive/libraries"/*.sdlibrary/library.json | grep '"id"'

# Run with env vars
export SD_LIBRARY_ID="your-library-uuid-here"
cargo run --release
```

## What You'll See

1. **Window opens** with "Spacedrive Media Grid" title
2. **Header** showing item count and grid settings
3. **Loading state** while querying daemon
4. **Grid of thumbnails** (up to 1000 photos/videos)
5. **Scrollable** - try scrolling to see all your media

## Current Limitations

This is a **proof of concept**, so:
- Loads max 1000 files (no pagination yet)
- Queries from root path `/`
- No virtual scrolling (renders all 1000 at once)
- No selection or interaction (view-only)
- No keyboard shortcuts
- Hard-coded 6 columns, 200px thumbnails

## What's Next

Phase 2 improvements:
- Virtual scrolling (only render visible rows)
- Keyboard navigation (arrow keys, space)
- Selection (click to select, shift-click for range)
- Right-click context menu
- Custom path browsing
- Column/size controls
- Performance testing with 10k+ images

## Troubleshooting

### "Error loading files"
- **Check daemon is running**: Open Spacedrive main app
- **Check socket exists**: `ls ~/.spacedrive/daemon.sock`
- **Check library ID is correct**: Run `./run.sh` to auto-detect

### "No media files found"
- The query looks for images/videos in path `/`
- Your media might be in a specific location
- Edit `SD_INITIAL_PATH` to point to your photos directory

### Window doesn't appear
- GPUI needs Metal (macOS) / Vulkan (Linux) / DirectX (Windows)
- Check terminal for errors
- Try running without `--release` to see debug output

### Images show "✗" (failed to load)
- HTTP server might not be running on port 54321
- Check `SD_HTTP_URL` environment variable
- Thumbnails might not be generated yet (run Spacedrive's indexer)

## Architecture Diagram

```
Your Terminal
    │
    └─> ./run.sh
            │
            ├─ Finds library ID from filesystem
            ├─ Sets env vars (SD_LIBRARY_ID, SD_SOCKET_PATH, SD_HTTP_URL)
            └─ Runs: cargo run --release
                    │
                    └─> GPUI App Window
                            │
                            ├─ Connects to ~/.spacedrive/daemon.sock
                            ├─ Sends: { "Query": { "method": "files.media_listing", ... } }
                            ├─ Receives: [{ file1 }, { file2 }, ...]
                            │
                            └─ For each file:
                                ├─ Extracts content_identity.uuid
                                ├─ Builds URL: http://127.0.0.1:54321/sidecar/{lib}/{uuid}/thumb/grid@1x.webp
                                └─ GPUI img() element fetches and renders
```

## Files You Created

```
apps/gpui-photo-grid/
├── Cargo.toml                  # Dependencies (gpui, sd-client)
├── src/
│   ├── main.rs                 # Entry point, window setup
│   └── photo_grid_view.rs      # Main view component
├── run.sh                      # Helper script (auto-finds library)
├── README.md                   # Full documentation
└── QUICKSTART.md              # This file
```

## Key Code Locations

**Loading files:** `photo_grid_view.rs:37` - `load_files()` method
**Grid rendering:** `photo_grid_view.rs:140` - `render_grid()` method
**Thumbnail URLs:** Uses `sd-client`'s `thumbnail_url()` method
**GPUI setup:** `main.rs:25` - Application creation with HTTP client

## Testing Ideas

1. **Open Spacedrive** - Make sure it's running
2. **Run the grid viewer** - `./run.sh`
3. **Check console output** - Should show "Loaded N files"
4. **Scroll the grid** - Should be smooth (all rendered, no virtualization yet)
5. **Compare with web UI** - Open MediaView in Spacedrive, compare feel
6. **Close and reopen** - Should reload fresh from daemon

## Next Session Goals

- [ ] Add virtual scrolling (big performance win)
- [ ] Keyboard navigation
- [ ] Test with 10k+ images
- [ ] Profile frame times
- [ ] Add selection support
- [ ] Context menu integration
