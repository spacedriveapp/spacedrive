# Spacedrive Server Architecture

## Overview

The Spacedrive Server is a production-ready HTTP server that embeds the Spacedrive daemon and serves the web interface. It's designed for headless deployments, NAS systems, and container environments.

## Design Principles

1. **Embedded Daemon** - No separate process management needed
2. **Single Binary** - Web assets bundled via `include_dir` when built with `--features assets`
3. **Platform Abstraction** - Uses same `@sd/interface` as Tauri, with web-specific platform impl
4. **Security First** - HTTP Basic Auth for all endpoints (except health check)
5. **Container Native** - Docker-first design with distroless runtime image

## Components

### 1. HTTP Server (`apps/server/src/main.rs`)

Built with Axum, provides:
- **`GET /health`** - Health check (no auth required)
- **`POST /rpc`** - JSON-RPC proxy to daemon Unix socket
- **`GET /*`** - Static asset serving (SPA fallback to index.html)

**Flow:**
```
Browser → HTTP Request → Axum Router → Basic Auth Middleware → Handler
                                                                    ↓
                                          ┌─────────────────────────┴────────┐
                                          ↓                                  ↓
                                    Static Assets                      RPC Proxy
                                    (serve from                         ↓
                                     ASSETS_DIR)              Unix Socket → Daemon
```

### 2. Embedded Daemon

Unlike Tauri (which spawns `sd-daemon` as a child process), the server runs the daemon in-process:

```rust
tokio::spawn(async move {
    sd_core::infra::daemon::bootstrap::start_default_server(
        socket_path,
        data_dir,
        enable_p2p,
    ).await
});
```

**Benefits:**
- Single container image
- Simplified lifecycle management
- Shared memory space (more efficient)
- Graceful shutdown via `tokio::select!`

**Daemon lifecycle:**
1. Check if socket already exists (reuse existing daemon)
2. If not, spawn daemon in background task
3. Wait for socket file to appear (max 3s)
4. Return handle for graceful shutdown

### 3. Web Client (`apps/web/`)

Minimal React app using `@sd/interface`:

```tsx
// apps/web/src/main.tsx
<PlatformProvider platform={webPlatform}>
  <Explorer />
</PlatformProvider>
```

**Platform implementation:**
```typescript
// apps/web/src/platform.ts
export const platform: Platform = {
  platform: "web",
  openLink(url) { window.open(url) },
  confirm(msg, cb) { cb(window.confirm(msg)) },
  // No native file pickers, daemon control, etc.
};
```

**Build process:**
1. Vite bundles React app → `apps/web/dist/`
2. `build.rs` runs `pnpm build` before compiling server
3. `include_dir!` macro embeds `dist/` into binary at compile time
4. Axum serves embedded files from memory

### 4. RPC Proxy

Browsers can't connect to Unix sockets, so the server proxies:

```
Browser               Server                    Daemon
   │                     │                         │
   │  POST /rpc          │                         │
   ├────────────────────>│                         │
   │  (JSON-RPC)         │  Unix Socket Write     │
   │                     ├────────────────────────>│
   │                     │                         │
   │                     │  Unix Socket Read       │
   │                     │<────────────────────────┤
   │  200 OK             │                         │
   │<────────────────────┤                         │
   │  (JSON-RPC result)  │                         │
```

**Implementation:**
```rust
async fn daemon_rpc(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut stream = UnixStream::connect(&state.socket_path).await?;
    stream.write_all(format!("{}\n", serde_json::to_string(&payload)?).as_bytes()).await?;

    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader.read_line(&mut response).await?;

    Ok(Json(serde_json::from_str(&response)?))
}
```

## Comparison: Server vs Tauri vs CLI

| Aspect | Server | Tauri | CLI |
|--------|--------|-------|-----|
| **Process Model** | Embedded daemon | Spawned daemon | Connects to daemon |
| **UI** | Web (React in browser) | WebView (React) | Terminal (TUI) |
| **Daemon Communication** | Unix socket (proxied) | Unix socket (direct) | Unix socket (direct) |
| **Platform Abstraction** | `platform: "web"` | `platform: "tauri"` | N/A |
| **Access Model** | Remote (HTTP) | Local only | Local only |
| **Auth** | HTTP Basic Auth | Not needed | Not needed |
| **Deployment** | Docker, systemd | App bundle | Binary |

## Authentication Flow

```
1. Browser makes request without auth
   ↓
2. basic_auth middleware checks state.auth
   ↓
3. If empty → allow (auth disabled)
   If populated → require Basic Auth header
   ↓
4. Extract credentials from Authorization header
   ↓
5. Compare with state.auth HashMap
   ↓
6. Match → proceed to handler
   No match → 401 Unauthorized
```

**Security considerations:**
- Credentials stored in memory as `SecStr` (zeroed on drop)
- Basic Auth over HTTPS recommended for production
- Socket file has filesystem permissions (only accessible to server user)

## Docker Architecture

### Multi-stage Build

```dockerfile
# Stage 1: Builder (Debian + Rust + Node)
FROM debian:bookworm-slim AS builder
RUN install Rust, Node, pnpm
COPY workspace
RUN pnpm build (web)
RUN cargo build --release --features assets

# Stage 2: Runtime (Distroless)
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=builder /build/target/release/sd-server
ENTRYPOINT ["/usr/bin/sd-server"]
```

**Benefits:**
- **Small image** - Distroless base (~50MB vs ~1GB for full Debian)
- **Secure** - No shell, no package manager, minimal attack surface
- **Fast** - Cached layers for dependencies
- **Reproducible** - Locked versions via `Cargo.lock` and `pnpm-lock.yaml`

### Volume Mounts

```yaml
volumes:
  - spacedrive-data:/data  # Persistent library data
  - /mnt/storage:/storage:ro  # Optional: Read-only media access
```

**Data layout:**
```
/data/
├── daemon/
│   └── daemon.sock        # Unix socket for RPC
├── libraries/
│   └── *.sdlibrary/       # SQLite databases
│       ├── library.db
│       └── sidecars/      # Thumbnails, previews
├── logs/
│   ├── daemon.log
│   └── indexing.log
└── current_library_id.txt
```

## Development vs Production

### Development Mode

```bash
# Terminal 1: Web dev server (hot reload)
cd apps/web
pnpm dev  # → http://localhost:3000

# Terminal 2: API server
cargo run -p sd-server
# → http://localhost:8080
# Vite proxies /rpc to 8080
```

**Workflow:**
1. Edit React components → Vite hot reloads
2. Edit server code → `cargo run` rebuilds
3. No need to rebuild web assets during development

### Production Build

```bash
# Build with bundled assets
cargo build --release -p sd-server --features assets

# Single binary contains:
# - Axum HTTP server
# - Embedded daemon
# - Bundled web UI (React app)
```

**Deployment:**
```bash
./target/release/sd-server \
  --data-dir /var/lib/spacedrive \
  --port 8080
```

## Platform Abstraction

Both Tauri and Web use `@sd/interface`, but with different platform implementations:

### Tauri Platform (`apps/tauri/src/platform.ts`)

```typescript
{
  platform: "tauri",
  openDirectoryPickerDialog: async () => open({ directory: true }),
  revealFile: async (path) => invoke("reveal_file", { path }),
  getCurrentLibraryId: async () => invoke("get_current_library_id"),
  getDaemonStatus: async () => invoke("get_daemon_status"),
  // Full native capabilities
}
```

### Web Platform (`apps/web/src/platform.ts`)

```typescript
{
  platform: "web",
  openLink: (url) => window.open(url),
  confirm: (msg, cb) => cb(window.confirm(msg)),
  // Minimal browser-only capabilities
}
```

**Interface components adapt:**
```tsx
function FilePickerButton() {
  const platform = usePlatform();

  if (platform.platform === "tauri") {
    // Show native picker button
    return <button onClick={platform.openDirectoryPickerDialog}>Pick</button>;
  } else {
    // Web: no native picker, show manual path input
    return <input type="text" placeholder="Enter path..." />;
  }
}
```

## Error Handling

### HTTP Errors

```rust
async fn daemon_rpc(...) -> Result<Json<Value>, (StatusCode, String)> {
    let stream = UnixStream::connect(&socket_path)
        .await
        .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, format!("Daemon not available: {}", e)))?;
    // ...
}
```

**Responses:**
- `503 Service Unavailable` - Daemon not running
- `400 Bad Request` - Invalid JSON
- `500 Internal Server Error` - RPC failed
- `401 Unauthorized` - Auth failed

### Daemon Errors

Daemon errors are passed through RPC response:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32603,
    "message": "Library not found"
  }
}
```

## Performance Considerations

1. **Static Assets** - Served from memory (embedded via `include_dir`)
2. **Socket Pooling** - Each RPC request opens new socket (TODO: connection pool)
3. **Async I/O** - Tokio runtime handles concurrent requests
4. **Graceful Shutdown** - Waits for in-flight requests before terminating

## Future Enhancements

1. **WebSocket Support** - Real-time event streaming (vs polling)
2. **HTTPS** - TLS termination (currently expects reverse proxy)
3. **Connection Pool** - Reuse Unix sockets for RPC
4. **Multi-tenancy** - Separate libraries per user
5. **SSE Events** - Server-sent events for daemon notifications

## Security Model

**Trust Boundaries:**
```
Internet ←[TLS]→ Reverse Proxy ←[HTTP+Auth]→ Server ←[Unix Socket]→ Daemon
  (❌)              (✅)                (✅)           (✅)            (✅)
```

**Assumptions:**
- Server runs on trusted network OR behind reverse proxy with TLS
- Unix socket accessible only to server process (filesystem permissions)
- HTTP Basic Auth sufficient for home/NAS use
- For public internet: Use nginx/Caddy with Let's Encrypt

## Monitoring

**Health Check:**
```bash
curl http://localhost:8080/health
# → "OK"
```

**Logs:**
```bash
# Docker
docker logs spacedrive -f

# Systemd
journalctl -u spacedrive -f

# Native
RUST_LOG=debug ./sd-server
```

**Metrics:** (TODO)
- Request count/latency
- Daemon socket errors
- Active connections

## Related Documentation

- [README.md](./README.md) - Setup and usage
- [../../docs/core/architecture.md](../../docs/core/architecture.md) - Core VDFS design
- [../tauri/DAEMON_SETUP.md](../tauri/DAEMON_SETUP.md) - Tauri daemon integration
