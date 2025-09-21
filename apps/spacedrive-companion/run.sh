#!/bin/bash

# Spacedrive Companion App Launch Script

set -e

echo "Starting Spacedrive SwiftUI Companion App..."

# Check if we're in the right directory
if [ ! -f "Package.swift" ]; then
    echo "Error: Package.swift not found. Please run this script from the spacedrive-companion directory."
    exit 1
fi

# Check if Spacedrive daemon socket exists
SOCKET_PATH="$HOME/Library/Application Support/spacedrive/daemon/daemon.sock"
if [ ! -S "$SOCKET_PATH" ]; then
    echo "⚠️  Warning: Spacedrive daemon socket not found at $SOCKET_PATH"
    echo "   Make sure the Spacedrive daemon is running before launching the companion app."
    echo "   You can start the daemon with: cargo run --bin sd-cli -- daemon"
    echo ""
fi

# Run the app
echo "🚀 Launching companion app..."
swift run SpacedriveCompanion
