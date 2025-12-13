#!/bin/bash
set -e

# This script fixes the daemon entitlements in the bundled macOS app
# It removes the app-sandbox entitlement which causes the daemon to crash

BUNDLE_PATH="$1"

if [ -z "$BUNDLE_PATH" ]; then
    echo "Usage: $0 <path-to-app-bundle>"
    exit 1
fi

DAEMON_PATH="$BUNDLE_PATH/Contents/MacOS/sd-daemon"
ENTITLEMENTS_PATH="$(dirname "$0")/../src-tauri/DaemonEntitlements.plist"

if [ ! -f "$DAEMON_PATH" ]; then
    echo "Error: Daemon not found at $DAEMON_PATH"
    exit 1
fi

if [ ! -f "$ENTITLEMENTS_PATH" ]; then
    echo "Error: DaemonEntitlements.plist not found at $ENTITLEMENTS_PATH"
    exit 1
fi

echo "Re-signing daemon with correct entitlements..."
codesign --force --sign - \
    --entitlements "$ENTITLEMENTS_PATH" \
    --options runtime \
    "$DAEMON_PATH"

echo "✓ Daemon re-signed successfully"
