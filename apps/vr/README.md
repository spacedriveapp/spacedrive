# Spacedrive VR

Native WebXR experience for Spacedrive.

## Architecture

The VR app runs in your VR headset's browser and connects to the Spacedrive daemon on your laptop:

```
VR Headset (Quest Browser)
    ↓ WebSocket
Proxy Server (your laptop)
    ↓ TCP Socket
Spacedrive Daemon (your laptop)
```

## Setup

### 1. Start the Spacedrive Daemon

```bash
# From workspace root
cargo run --bin sd-daemon
```

The daemon will listen on `127.0.0.1:6969`.

### 2. Start the Proxy Server

The proxy bridges WebSocket (from VR) to TCP socket (daemon):

```bash
cd apps/vr
bun run proxy
```

The proxy will:

- Listen on `https://0.0.0.0:8080` (HTTPS/WSS with self-signed cert)
- Auto-generate a self-signed certificate on first run (requires OpenSSL)
- Store certificates in `.certs/` directory

### 3. Update VR App Configuration

Edit `src/App.tsx` and set your laptop's LAN IP address:

```typescript
const PROXY_WS_URL = "wss://192.168.0.91:8080/ws"; // Change to your IP
```

**Important:** Use `wss://` (secure WebSocket), not `ws://`, because the VR page loads over HTTPS.

To find your laptop's IP:

- **macOS**: `ipconfig getifaddr en0`
- **Linux**: `ip addr show`
- **Windows**: `ipconfig`

### 4. Start the VR App (Dev Mode)

```bash
bun run dev
```

This starts Vite with `--host` to make it accessible on your network.

### 5. Access from VR Headset

**Important:** Accept certificates in this order:

1. Put on your VR headset and open the browser
2. **First**, navigate to: `https://<your-laptop-ip>:8080/test`
   - Accept the proxy server's certificate warning
   - Click "Test WebSocket" to verify connection works
   - You should see "✅ WebSocket connected!" in the log
3. **Then**, navigate to: `https://<your-laptop-ip>:5173`
   - Accept the Vite dev server's certificate warning
4. Click "Enter VR"

**Why two certificates?** The VR app (Vite) and proxy server (WebSocket) run on different ports, so each needs its certificate accepted.

## Development

### Running All Services

You'll need 3 terminals:

```bash
# Terminal 1: Daemon
cargo run --bin sd-daemon

# Terminal 2: Proxy Server
cd apps/vr && bun run proxy

# Terminal 3: VR Dev Server
cd apps/vr && bun run dev
```

### Checking Connection

Test the proxy server:

```bash
curl http://localhost:8080/health
```

Should return:

```json
{ "status": "ok", "daemon": "127.0.0.1:6969" }
```

### Debugging

- **Browser console** (F12 in Quest browser): See WebSocket logs
- **Proxy server terminal**: See message flow between VR and daemon
- **Daemon logs**: See daemon processing

## Implementation Notes

### Why a Proxy Server?

The Spacedrive daemon uses TCP sockets for communication (following the Tauri app's approach). Browsers cannot make raw TCP connections due to security restrictions, so we need a WebSocket-to-TCP proxy.

### WebSocket Protocol

The proxy uses secure WebSocket (WSS) with a self-signed certificate and forwards JSON-RPC messages as newline-delimited JSON:

```
VR (wss://) → Proxy (TLS) → Daemon (TCP): {"Query":{"method":"query:libraries.list","library_id":null,"payload":{"include_stats":true}}}
Daemon → Proxy → VR: {"JsonOk":[{"id":"abc...","name":"My Library","path":"/path/to/library","stats":{...}}]}
```

**Security Notes:**

- Proxy uses self-signed certificate (you'll need to accept the certificate warning in your VR browser)
- For production, replace with a proper SSL certificate
- All traffic between VR and proxy is encrypted via TLS

### Native VR UI

The VR interface uses native Three.js/WebXR components instead of rendering the React interface. This provides:

- Better performance (no HTML re-rendering)
- True VR interactions (raycasting, controllers)
- Spatial UI optimized for VR

## Current Features

- ✅ Connect to Spacedrive daemon via WebSocket proxy
- ✅ Display library information in VR space
- ✅ Native VR file explorer with Spacedrive color scheme
  - Left sidebar: Locations list
  - Right panel: File grid view (up to 100 files)
  - Click to cycle through locations (temporary until raycasting)
- ✅ Canvas texture rendering for native VR UI
- ✅ Real-time data with useNormalizedQuery hooks
- 🚧 VR controller raycasting for precise interactions
- 🚧 3D file previews (splats, models, images)
- 🚧 File operations (open, copy, move, delete)

## Troubleshooting

### "Failed to fetch" error

1. Check daemon is running: `cargo run --bin sd-daemon`
2. Check proxy is running: `bun run proxy`
3. Check IP address in `src/App.tsx` is correct
4. Check VR headset and laptop are on same network

### WebSocket connection fails

1. Verify proxy server started successfully
2. Check firewall isn't blocking port 8080
3. Try accessing `http://<laptop-ip>:8080/health` from VR browser

### WebSocket connection fails

**Most common issue:** Certificate not accepted for the proxy server.

**Solution:**

1. In your VR browser, visit `https://<laptop-ip>:8080/test`
2. Accept the certificate warning
3. Click "Test WebSocket" button
4. You should see "✅ WebSocket connected!"
5. If test passes, reload the VR app

**Still failing?**

- Check proxy server is running: `bun run proxy`
- Verify IP address in `src/App.tsx` matches your laptop
- Check firewall isn't blocking port 8080
- Look at proxy server terminal for connection attempts

## Next Steps

- [ ] Implement native VR file explorer
- [ ] Add 3D previews for common file types
- [ ] VR controller interactions (grab, move files)
- [ ] Spatial organization of files
- [ ] Integration with Gaussian splat viewer
