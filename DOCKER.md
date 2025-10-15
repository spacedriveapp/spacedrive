# Spacedrive Docker Deployment

Quick guide for running Spacedrive daemon in Docker.

## Quick Start

```bash
# Build the image
docker compose build

# Start daemon
docker compose up -d

# View logs
docker compose logs -f

# Check status
docker exec spacedrive-daemon sd-cli status
```

## Supported Platforms

- **x86_64** (amd64) - Servers, TrueNAS, Intel/AMD systems
- **ARM64** (aarch64) - Raspberry Pi 3/4/5, Apple Silicon (via emulation)

## Configuration

Edit `docker-compose.yml` to customize:

```yaml
volumes:
  # Mount directories to index
  - /path/to/your/photos:/mnt/photos:ro
  - /path/to/your/documents:/mnt/docs:ro

environment:
  # Optional: Set instance name
  - SPACEDRIVE_INSTANCE=myserver
```

## CLI Access

```bash
# Run any CLI command
docker exec spacedrive-daemon sd-cli <command>

# Examples:
docker exec spacedrive-daemon sd-cli library list
docker exec spacedrive-daemon sd-cli location add /mnt/photos
docker exec spacedrive-daemon sd-cli search "vacation"
```

## Data Persistence

Data is stored in the `spacedrive-data` Docker volume. To backup:

```bash
# Backup volume
docker run --rm -v spacedrive-data:/data -v $(pwd):/backup \
  alpine tar czf /backup/spacedrive-backup.tar.gz /data

# Restore volume
docker run --rm -v spacedrive-data:/data -v $(pwd):/backup \
  alpine tar xzf /backup/spacedrive-backup.tar.gz -C /
```

## Building for Specific Platform

```bash
# Build for ARM64 (Raspberry Pi)
docker build --platform linux/arm64 -t spacedrive:arm64 .

# Build for x86_64
docker build --platform linux/amd64 -t spacedrive:amd64 .
```

## TrueNAS Deployment

1. Enable Apps in TrueNAS SCALE
2. Create custom app using the provided `docker-compose.yml`
3. Mount your pools as volumes:
   ```yaml
   volumes:
     - /mnt/pool1:/mnt/pool1:ro
     - /mnt/pool2:/mnt/pool2:ro
   ```

## Troubleshooting

### Container exits immediately

Check logs:
```bash
docker compose logs
```

### Can't access daemon

Verify it's running:
```bash
docker ps
docker exec spacedrive-daemon sd-cli status
```

### Out of disk space

Check Docker disk usage:
```bash
docker system df
```

Clean up old data:
```bash
docker system prune
```

## Advanced

### Resource Limits

Add to `docker-compose.yml`:

```yaml
deploy:
  resources:
    limits:
      cpus: '2.0'
      memory: 2G
```

### Custom Data Directory

```yaml
volumes:
  # Use host directory instead of volume
  - /path/to/spacedrive/data:/data
```

### Network Access (Future API)

```yaml
ports:
  - "8080:8080"  # Expose API port
```

## Full Documentation

See [Linux Deployment Guide](./docs/cli/linux-deployment.mdx) for complete documentation including:
- Native binary installation
- Systemd service setup
- Raspberry Pi specific configuration
- TrueNAS integration
- Performance tuning

## Getting Help

- Documentation: https://docs.spacedrive.com
- GitHub Issues: https://github.com/spacedriveapp/spacedrive/issues
