#!/bin/bash
set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
TAURI_ROOT="$SCRIPT_DIR/.."
ICON_SOURCE="$TAURI_ROOT/Spacedrive.icon"
GEN_DIR="$TAURI_ROOT/src-tauri/gen"

# Create gen directory if it doesn't exist
mkdir -p "$GEN_DIR"

# Compile .icon to Assets.car using actool
echo "Compiling Spacedrive.icon to Assets.car..."
xcrun actool "$ICON_SOURCE" \
  --compile "$GEN_DIR" \
  --output-format human-readable-text \
  --notices --warnings --errors \
  --output-partial-info-plist "$GEN_DIR/partial.plist" \
  --app-icon Spacedrive \
  --include-all-app-icons \
  --enable-on-demand-resources NO \
  --development-region en \
  --target-device mac \
  --minimum-deployment-target 11.0 \
  --platform macosx

echo "Successfully generated Assets.car and Spacedrive.icns"
