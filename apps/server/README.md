# Spacedrive Server

HTTP server for Spacedrive with embedded daemon and web interface.

## Overview

`sd-server` runs the Spacedrive daemon and serves a web interface over HTTP. Perfect for:
- **NAS deployments** (TrueNAS, Unraid, Synology, etc.)
- **Headless servers**
- **Remote access** to your Spacedrive libraries
- **Docker/container environments**

## Architecture

```
┌─────────────────────────────────────────┐
│         sd-server (HTTP Server)         │
│  ┌───────────────────────────────────┐  │
│  │  Axum HTTP Server (Port 8080)     │  │
│  │  ├─ /health (healthcheck)         │  │
│  │  ├─ /rpc (proxy to daemon)        │  │
│  │  └─ /* (web UI assets)            │  │
│  └───────────────────────────────────┘  │
│               ↓                          │
│  ┌───────────────────────────────────┐  │
│  │  Embedded Daemon                  │  │
│  │  (Unix socket: daemon.sock)       │  │
│  │  ├─ Core VDFS                     │  │
│  │  ├─ Indexing                      │  │
│  │  ├─ P2P Networking                │  │
│  │  └─ File Operations               │  │
│  └───────────────────────────────────┘  │
└─────────────────────────────────────────┘
```

Unlike Tauri (desktop app), the server:
- Embeds the daemon instead of spawning a separate process
- Serves the web UI as static assets (when built with `--features assets`)
- Proxies RPC requests from browser to daemon via Unix socket
- Provides basic auth for security

## Quick Start

### Development (without Docker)

1. **Build the server:**
   ```bash
   # Without web assets (for API-only usage)
   cargo build -p sd-server

   # With bundled web UI
   cargo build -p sd-server --features assets
   ```

2. **Run the server:**
   ```bash
   # Development mode (creates temp data dir)
   cargo run -p sd-server

   # Production mode (requires DATA_DIR)
   DATA_DIR=/path/to/data cargo run -p sd-server --release --features assets
   ```

3. **Access the web UI:**
   - Open http://localhost:8080
   - Default auth: disabled in dev mode

### Docker Deployment (Recommended)

Perfect for TrueNAS, Unraid, or any Docker-compatible NAS.

1. **Create a `.env` file:**
   ```bash
   # REQUIRED: Set your credentials
   SD_AUTH=admin:your-secure-password

   # Optional: Change port
   PORT=8080

   # Optional: Disable auth (NOT RECOMMENDED)
   # SD_AUTH=disabled
   ```

2. **Start with docker-compose:**
   ```bash
   cd apps/server
   docker-compose up -d
   ```

3. **Access the server:**
   - Navigate to `http://your-nas-ip:8080`
   - Login with credentials from `.env`

## Configuration

### Environment Variables

| Variable | Description | Default | Required |
|----------|-------------|---------|----------|
| `DATA_DIR` | Path to Spacedrive data directory | `/data` (in Docker) | Yes (production) |
| `PORT` | HTTP server port | `8080` | No |
| `SD_AUTH` | Authentication credentials (format: `user:pass,user2:pass2`) | None | Recommended |
| `SD_P2P` | Enable P2P networking | `true` | No |
| `RUST_LOG` | Log level | `info,sd_core=debug` | No |

### Authentication

**IMPORTANT:** Always set `SD_AUTH` in production!

```bash
# Single user
SD_AUTH=admin:securepassword123

# Multiple users
SD_AUTH=admin:pass1,user:pass2,readonly:pass3

# Disable (NOT RECOMMENDED - only for trusted networks)
SD_AUTH=disabled
```

Uses HTTP Basic Authentication. The server will return `401 Unauthorized` if credentials don't match.

### Data Storage

The server stores all data in `DATA_DIR`:
```
$DATA_DIR/
├── daemon/
│   └── daemon.sock          # Unix socket for RPC
├── libraries/
│   └── *.sdlibrary/         # Library databases
├── logs/                     # Application logs
└── current_library_id.txt   # Last opened library
```

**Docker volumes:** Mounted at `/data` inside the container.

## TrueNAS Setup

### Using TrueNAS SCALE (Docker)

1. **Navigate to Apps** in TrueNAS web UI
2. **Click "Launch Docker Image"**
3. **Configure:**
   - **Image:** Build locally or use pre-built image
   - **Port:** Map `8080` to host
   - **Volume:** Mount `/mnt/pool/spacedrive` to `/data`
   - **Environment:**
     - `SD_AUTH=admin:yourpassword`
     - `TZ=America/New_York` (your timezone)

4. **Add storage pools** (optional):
   - Mount your datasets as read-only volumes
   - Example: `/mnt/tank/photos` → `/photos` in container

### Manual Docker Run

```bash
docker run -d \
  --name spacedrive \
  -p 8080:8080 \
  -p 7373:7373 \
  -v /mnt/pool/spacedrive:/data \
  -v /mnt/pool/media:/media:ro \
  -e SD_AUTH=admin:password \
  -e TZ=UTC \
  --restart unless-stopped \
  spacedrive/server:latest
```

## Building

### With Web Assets (Production)

```bash
# Build everything (web UI + server)
cargo build --release -p sd-server --features assets

# The binary includes bundled web assets
./target/release/sd-server --data-dir /path/to/data
```

### Without Assets (API Only)

```bash
# Build server without web UI
cargo build --release -p sd-server

# Serve API endpoints only
./target/release/sd-server --data-dir /path/to/data
```

In this mode, you can connect with:
- `sd-cli` (CLI client)
- Custom HTTP clients via `/rpc`
- Tauri desktop app configured to connect to this server

## Development Workflow

1. **Run web dev server:**
   ```bash
   cd apps/web
   pnpm dev
   ```
   This starts Vite on http://localhost:3000 with hot reload.

2. **Run API server:**
   ```bash
   cargo run -p sd-server
   ```
   This starts the HTTP server on http://localhost:8080.

3. **Develop:**
   - Edit React components in `apps/web/src`
   - Edit server code in `apps/server/src`
   - Vite proxies `/rpc` requests to the server

## API Endpoints

### `GET /health`
Health check endpoint.

**Response:** `200 OK` with body `"OK"`

### `POST /rpc`
JSON-RPC proxy to daemon.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "query:libraries.list",
  "params": { "include_stats": false }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": [...]
}
```

### `GET /*` (with `--features assets`)
Serves the bundled web UI. All non-API routes fallback to `index.html` for SPA routing.

## Comparison: Server vs Tauri

| Feature | Server | Tauri |
|---------|--------|-------|
| **Platform** | Linux/Docker | macOS/Windows/Linux |
| **UI** | Web (React in browser) | Native webview |
| **Daemon** | Embedded in process | Spawned as child process |
| **Access** | Remote over HTTP | Local only |
| **Auth** | HTTP Basic Auth | Not needed (local) |
| **Use Case** | NAS, headless servers | Desktop workstations |

Both use the same Spacedrive core and `@sd/interface` package!

## Troubleshooting

### Server won't start
- Check `DATA_DIR` exists and is writable
- Verify port 8080 is not in use: `lsof -i :8080`
- Check logs: `RUST_LOG=debug cargo run -p sd-server`

### Can't connect to daemon
- Ensure `daemon.sock` exists in `$DATA_DIR/daemon/`
- Check daemon logs in `$DATA_DIR/logs/`
- Try removing stale socket: `rm $DATA_DIR/daemon/daemon.sock`

### Authentication failing
- Verify `SD_AUTH` format: `username:password`
- Check browser is sending Basic Auth header
- Test with curl:
  ```bash
  curl -u admin:password http://localhost:8080/health
  ```

### Docker build failing
- Ensure you're building from repository root:
  ```bash
  docker build -f apps/server/Dockerfile .
  ```
- Check Docker has enough memory (4GB+ recommended)

## Contributing

The server app is part of the Spacedrive v2 monorepo.

**Project structure:**
```
apps/server/
├── src/
│   └── main.rs          # Server implementation
├── Cargo.toml           # Dependencies
├── build.rs             # Web bundling script
├── Dockerfile           # Container image
└── docker-compose.yml   # Docker setup

apps/web/
├── src/
│   ├── main.tsx         # Web entry point
│   └── platform.ts      # Web platform implementation
├── package.json
└── vite.config.ts
```

**Making changes:**
1. Server code: Edit `apps/server/src/main.rs`
2. Web UI: Edit `apps/web/src/*` (uses `@sd/interface`)
3. Platform integration: Edit `apps/web/src/platform.ts`

## License

AGPL-3.0 - See LICENSE file in repository root.
