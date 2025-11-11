#!/bin/bash
set -e

# Bundle libheif for local development
# This script copies libheif.dylib from Homebrew to the target directory

TARGET_DIR="${1:-target/debug}"
LIBHEIF_SRC="/opt/homebrew/lib/libheif.1.dylib"

if [ ! -f "$LIBHEIF_SRC" ]; then
    echo "Error: libheif not found at $LIBHEIF_SRC"
    echo "Install with: brew install libheif"
    exit 1
fi

# Create target directory if it doesn't exist
mkdir -p "$TARGET_DIR"

# Copy libheif and its dependencies
echo "Copying libheif to $TARGET_DIR..."
cp -f "$LIBHEIF_SRC" "$TARGET_DIR/libheif.1.dylib"

# Create symlink for compatibility
ln -sf libheif.1.dylib "$TARGET_DIR/libheif.dylib"

echo "✓ libheif bundled successfully"
echo "  Location: $TARGET_DIR/libheif.1.dylib"
