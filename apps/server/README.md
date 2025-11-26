# Spacedrive Server

HTTP server for Spacedrive with embedded daemon (RPC only, no web UI).

## Overview

`sd-server` runs the Spacedrive daemon and exposes RPC endpoints over HTTP. Perfect for:
- **NAS deployments** (TrueNAS, Unraid, Synology, etc.)
- **Headless servers**
- **Remote access** to your Spacedrive libraries
- **Docker/container environments**
- **CLI-only usage** with `sd-cli`

## Architecture

```
┌─────────────────────────────────────────┐
│         sd-server (HTTP Server)         │
│  ┌───────────────────────────────────┐  │
│  │  Axum HTTP Server (Port 8080)     │  │
│  │  ├─ /health (healthcheck)         │  │
│  │  └─ /rpc (proxy to daemon)        │  │
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
- Provides only RPC endpoints (no bundled web UI)
- Proxies RPC requests to daemon via Unix socket
- Provides basic auth for security

## Quick Start

### Development (without Docker)

1. **Build the server:**
   ```bash
   cargo build -p sd-server
   ```

2. **Run the server:**
   ```bash
   # Development mode (creates temp data dir)
   cargo run -p sd-server

   # Production mode (requires DATA_DIR)
   DATA_DIR=/path/to/data cargo run -p sd-server --release
   ```

3. **Access the RPC endpoint:**
   - Health check: http://localhost:8080/health
   - RPC endpoint: http://localhost:8080/rpc
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

```bash
# Build server (RPC only)
cargo build --release -p sd-server

# Run server
./target/release/sd-server --data-dir /path/to/data
```

You can connect with:
- `sd-cli` (CLI client)
- Custom HTTP clients via `/rpc`
- Tauri desktop app configured to connect to this server
- Future web UI (not yet implemented)

## Development Workflow

```bash
# Run server in dev mode
cargo run -p sd-server

# Server starts on http://localhost:8080
# Use sd-cli or custom client to interact with RPC endpoint
```

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


## Comparison: Server vs Tauri

| Feature | Server | Tauri |
|---------|--------|-------|
| **Platform** | Linux/Docker | macOS/Windows/Linux |
| **UI** | None (RPC only) | Native webview |
| **Daemon** | Embedded in process | Spawned as child process |
| **Access** | Remote over HTTP | Local only |
| **Auth** | HTTP Basic Auth | Not needed (local) |
| **Use Case** | NAS, headless servers, CLI | Desktop workstations |

Both use the same Spacedrive core!

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
├── Dockerfile           # Container image
└── docker-compose.yml   # Docker setup
```

**Making changes:**
1. Server code: Edit `apps/server/src/main.rs`
2. Daemon integration: See `core/src/infra/daemon/`

## License

AGPL-3.0 - See LICENSE file in repository root.
