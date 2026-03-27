# Server Release Setup

This document explains the changes made to integrate Spacedrive server builds into the release workflow.

## Overview

The server app (`sd-server`) is now built and released in two formats:
1. **Static binaries** - For systemd, bare metal, and custom deployments
2. **Docker images** - For containerized deployments (Docker, Kubernetes, NAS systems)

Both are automatically built and published when a git tag is pushed (e.g., `v2.0.0-alpha.2`).

**Note:** The root `/Dockerfile` has been removed as it was redundant. The only Docker image for self-hosting is `apps/server/Dockerfile`, which builds the HTTP server with embedded daemon and full media processing support.

## Changes Made

### 1. Release Workflow (`.github/workflows/release.yml`)

**Added: `server-build` job**

Builds static server binaries for:
- `linux-x86_64` (Intel/AMD servers)
- `linux-aarch64` (ARM servers, Raspberry Pi, AWS Graviton)

**Features:**
- Full media processing support (`heif`, `ffmpeg` features enabled)
- Cross-compilation for ARM using `gcc-aarch64-linux-gnu`
- Checksums generated for each binary (SHA256)
- Archives created as `.tar.gz` for easy distribution

**Artifacts uploaded:**
- `sd-server-linux-x86_64.tar.gz`
- `sd-server-linux-aarch64.tar.gz`

**Updated: `release` job**

- Added `server-build` to dependencies
- Server artifacts now included in GitHub releases
- Pattern updated to include `.tar.gz` files

### 2. Server Docker Workflow (`.github/workflows/server.yml`)

**Changed trigger:**
- Old: `release: types: [published]`
- New: `push: tags: ["v*"]`
- Result: Docker images built at the same time as binaries

**Multi-arch support:**
- Old: `amd64` only
- New: `amd64` + `arm64`
- Uses QEMU for ARM cross-compilation

**Fixed paths:**
- Old: `context: ./apps/server/docker` (incorrect)
- New: `context: .` (repo root)
- Old: `containerfiles: ./apps/server/docker/Dockerfile` (incorrect)
- New: `containerfiles: ./apps/server/Dockerfile` (correct)

**Image tagging:**
- Git tags (e.g., `v2.0.0-alpha.2`) → `ghcr.io/spacedriveapp/spacedrive/server:v2.0.0-alpha.2` + `latest`
- Non-tagged commits → `ghcr.io/spacedriveapp/spacedrive/server:<commit-sha>` + `staging`

### 3. Server Dockerfile (`apps/server/Dockerfile`)

**Added media processing dependencies:**

Builder stage:
- `cmake`, `nasm` - Build tools for native dependencies
- `libavcodec-dev`, `libavformat-dev`, `libavutil-dev`, `libswscale-dev` - FFmpeg dev libraries
- `libheif-dev` - HEIF image format support

Runtime stage:
- Changed from `distroless/cc` to `debian:bookworm-slim`
- Installed runtime libraries: `libavcodec59`, `libavformat59`, `libavutil57`, `libswscale6`, `libheif1`
- Created `spacedrive` user (UID 1000) for security

**Enabled features in build:**
```dockerfile
cargo build --release -p sd-server --features sd-core/heif,sd-core/ffmpeg
```

This enables:
- Video thumbnail generation
- Audio transcription
- HEIF/HEIC image support
- All media processing capabilities

## Release Process

### Automated Release (Recommended)

1. **Tag a release:**
   ```bash
   git tag v2.0.0-alpha.2
   git push origin v2.0.0-alpha.2
   ```

2. **GitHub Actions automatically:**
   - Builds server binaries (x86_64 + ARM)
   - Builds desktop apps (macOS + Linux)
   - Builds Docker images (amd64 + arm64)
   - Creates draft GitHub release with all artifacts

3. **Review and publish:**
   - Go to GitHub Releases
   - Edit the draft release
   - Add release notes
   - Publish

### Manual Testing

**Test static binary build:**
```bash
# From project root
cargo build --release -p sd-server --features sd-core/heif,sd-core/ffmpeg

# Test locally
./target/release/sd-server --data-dir /tmp/sd-test
```

**Test Docker build:**
```bash
# From project root
docker build -f apps/server/Dockerfile -t sd-server-test .

# Run locally
docker run -p 8080:8080 -e SD_AUTH=admin:test sd-server-test
```

**Test multi-arch Docker build:**
```bash
docker buildx create --use
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -f apps/server/Dockerfile \
  -t sd-server-multiarch \
  .
```

## Deployment Options

### Option 1: Static Binary (systemd)

```bash
# Download from GitHub release
wget https://github.com/spacedriveapp/spacedrive/releases/download/v2.0.0-alpha.2/sd-server-linux-x86_64.tar.gz
tar -xzf sd-server-linux-x86_64.tar.gz

# Verify checksum
sha256sum -c sd-server-linux-x86_64.sha256

# Install
sudo mv sd-server-linux-x86_64 /usr/local/bin/sd-server
sudo chmod +x /usr/local/bin/sd-server

# Create systemd service
sudo nano /etc/systemd/system/spacedrive.service
```

Example systemd unit:
```ini
[Unit]
Description=Spacedrive Server
After=network.target

[Service]
Type=simple
User=spacedrive
Environment="DATA_DIR=/var/lib/spacedrive"
Environment="SD_AUTH=admin:your-secure-password"
ExecStart=/usr/local/bin/sd-server --data-dir /var/lib/spacedrive
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

### Option 2: Docker

```bash
docker run -d \
  --name spacedrive \
  -p 8080:8080 \
  -p 7373:7373 \
  -v spacedrive-data:/data \
  -e SD_AUTH=admin:password \
  ghcr.io/spacedriveapp/spacedrive/server:latest
```

### Option 3: Docker Compose

Use the provided `docker-compose.yml` in `apps/server/`:

```bash
cd apps/server
docker-compose up -d
```

Or create your own:

```yaml
version: '3.8'
services:
  spacedrive:
    image: ghcr.io/spacedriveapp/spacedrive/server:latest
    ports:
      - "8080:8080"
      - "7373:7373"
    volumes:
      - spacedrive-data:/data
      - /mnt/storage:/storage:ro  # Optional: mount storage
    environment:
      SD_AUTH: "admin:your-password"
      TZ: "America/New_York"
    restart: unless-stopped

volumes:
  spacedrive-data:
```

## Architecture Support

| Platform | Binary | Docker |
|----------|--------|--------|
| Linux x86_64 (Intel/AMD) | ✅ | ✅ |
| Linux ARM64 (Raspberry Pi, AWS Graviton) | ✅ | ✅ |
| macOS | ❌ (desktop app only) | ❌ |
| Windows | ❌ (desktop app only) | ❌ |

## Verify Release Artifacts

After a release is created, verify these files exist:

**Server binaries:**
- `sd-server-linux-x86_64.tar.gz`
- `sd-server-linux-aarch64.tar.gz`

**Desktop apps:**
- `Spacedrive_<version>_aarch64.dmg` (macOS ARM)
- `Spacedrive_<version>_amd64.deb` (Linux)
- `dist.tar.xz` (frontend assets)

**Docker images:**
Check ghcr.io:
```bash
docker pull ghcr.io/spacedriveapp/spacedrive/server:v2.0.0-alpha.2
docker pull ghcr.io/spacedriveapp/spacedrive/server:latest
```

## Next Steps

### Potential Enhancements

1. **Add Windows server binary** - Build `sd-server.exe` for Windows Server deployments
2. **Package formats** - Create `.deb` and `.rpm` packages for easier installation
3. **ARM macOS** - Server binary for macOS (though desktop app is preferred)
4. **Static linking** - Fully static binaries using `musl` for maximum compatibility
5. **Checksums in release notes** - Auto-generate checksums table in release description

### Documentation Updates Needed

- [x] Add "Self-Hosting Guide" to docs (`docs/overview/self-hosting.mdx`)
- [x] Update main README with server deployment links
- [ ] Create TrueNAS app manifest for one-click install
- [ ] Write Unraid template

## Troubleshooting

### Build fails with "media features not found"

The workflow now automatically includes `sd-core/heif` and `sd-core/ffmpeg` features. If this fails:
- Check native dependencies are installed (cmake, nasm, FFmpeg dev packages)
- Verify setup-system action runs successfully

### Docker image size too large

Current runtime image uses `debian:bookworm-slim` (~80-100MB base) plus runtime libraries.

To reduce size:
- Consider Alpine Linux base (smaller but more complex dependencies)
- Use distroless and manually copy .so files (more brittle)
- Current approach prioritizes reliability over size

### Multi-arch build timeout

ARM builds can be slow via QEMU emulation. Options:
- Use native ARM runners (more expensive)
- Build in parallel jobs
- Cache build artifacts more aggressively

### Permission denied in container

The container runs as user `spacedrive` (UID 1000). If mounting volumes:
```bash
# Fix permissions
sudo chown -R 1000:1000 /path/to/data
```

Or run as root (not recommended):
```bash
docker run --user root ...
```
