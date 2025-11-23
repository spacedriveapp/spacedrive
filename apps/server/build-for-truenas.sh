#!/bin/bash
# Build Spacedrive Server for TrueNAS deployment
# Usage: ./build-for-truenas.sh [truenas-ip]

set -e

TRUENAS_IP="${1:-}"
IMAGE_NAME="spacedrive-server"
IMAGE_TAG="latest"
TAR_FILE="spacedrive-server-$(date +%Y%m%d-%H%M%S).tar.gz"

echo "🏗️  Building Spacedrive Server for linux/amd64..."
echo ""

# Check if buildx is available
if ! docker buildx version &> /dev/null; then
    echo "❌ docker buildx not found. Installing..."
    docker buildx create --use
fi

# Build the image for linux/amd64 (TrueNAS architecture)
echo "📦 Building Docker image..."
cd ../..  # Go to repository root

docker buildx build \
    --platform linux/amd64 \
    -f apps/server/Dockerfile \
    -t ${IMAGE_NAME}:${IMAGE_TAG} \
    --load \
    .

echo ""
echo "✅ Build complete!"
echo ""

# Save the image to a tar file
echo "💾 Saving image to ${TAR_FILE}..."
docker save ${IMAGE_NAME}:${IMAGE_TAG} | gzip > "apps/server/${TAR_FILE}"

IMAGE_SIZE=$(du -h "apps/server/${TAR_FILE}" | cut -f1)
echo "✅ Image saved: apps/server/${TAR_FILE} (${IMAGE_SIZE})"
echo ""

# If TrueNAS IP provided, offer to transfer
if [ -n "$TRUENAS_IP" ]; then
    echo "📤 Transfer to TrueNAS?"
    echo "   Target: root@${TRUENAS_IP}"
    echo ""
    read -p "Continue? (y/n) " -n 1 -r
    echo

    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo "Transferring..."
        scp "apps/server/${TAR_FILE}" "root@${TRUENAS_IP}:/tmp/"

        echo ""
        echo "✅ Transfer complete!"
        echo ""
        echo "📋 Next steps on TrueNAS:"
        echo "   1. SSH into TrueNAS: ssh root@${TRUENAS_IP}"
        echo "   2. Load the image: gunzip -c /tmp/${TAR_FILE} | docker load"
        echo "   3. Deploy via TrueNAS SCALE Apps UI (see instructions below)"
        echo ""
    fi
else
    echo "📋 Manual deployment steps:"
    echo ""
    echo "1. Transfer the image to TrueNAS:"
    echo "   scp apps/server/${TAR_FILE} root@YOUR-TRUENAS-IP:/tmp/"
    echo ""
    echo "2. SSH into TrueNAS:"
    echo "   ssh root@YOUR-TRUENAS-IP"
    echo ""
    echo "3. Load the image:"
    echo "   gunzip -c /tmp/${TAR_FILE} | docker load"
    echo ""
    echo "4. Verify it loaded:"
    echo "   docker images | grep spacedrive"
    echo ""
fi

echo "═══════════════════════════════════════════════════════════════"
echo "🎯 TrueNAS SCALE GUI Deployment Instructions"
echo "═══════════════════════════════════════════════════════════════"
echo ""
echo "After loading the image on TrueNAS:"
echo ""
echo "1. Go to: Apps → Discover Apps → Launch Docker Image"
echo ""
echo "2. Container Settings:"
echo "   • Image Repository: ${IMAGE_NAME}"
echo "   • Image Tag: ${IMAGE_TAG}"
echo "   • Image Pull Policy: Never (important - use local image!)"
echo "   • Container Name: spacedrive"
echo ""
echo "3. Port Forwarding:"
echo "   • Container Port: 8080 → Node Port: 8080"
echo "   • Container Port: 7373 → Node Port: 7373"
echo ""
echo "4. Storage (Host Path Volumes):"
echo "   • Host Path: /mnt/YOUR-POOL/spacedrive"
echo "     Mount Path: /data"
echo "     Type: ixVolume (or Host Path)"
echo ""
echo "   Optional - Mount your media:"
echo "   • Host Path: /mnt/YOUR-POOL/media"
echo "     Mount Path: /media"
echo "     Read Only: ✓"
echo ""
echo "5. Environment Variables:"
echo "   • SD_AUTH = admin:changeme (CHANGE THIS!)"
echo "   • TZ = America/New_York (your timezone)"
echo "   • RUST_LOG = info,sd_core=debug"
echo ""
echo "6. Health Check (optional but recommended):"
echo "   • Type: HTTP"
echo "   • Port: 8080"
echo "   • Path: /health"
echo ""
echo "7. Click 'Install'"
echo ""
echo "═══════════════════════════════════════════════════════════════"
echo ""
echo "🌐 Access: http://YOUR-TRUENAS-IP:8080"
echo "🔐 Login with credentials from SD_AUTH"
echo ""
