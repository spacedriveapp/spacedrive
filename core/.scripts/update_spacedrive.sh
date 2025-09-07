#!/bin/bash

# --- Configuration ---
PROJECT_ROOT="/Users/jamespine/Projects/spacedrive/core"
INSTALL_DIR="/usr/local/bin"
BINARY_NAME="spacedrive"

# --- Script Logic ---

echo "Updating Spacedrive CLI..."

# Navigate to the project root
cd "$PROJECT_ROOT" || { echo "Error: Could not navigate to project root: $PROJECT_ROOT"; exit 1; }

# Pull latest changes from Git
echo "Pulling latest changes from Git..."
git pull || { echo "Error: Git pull failed."; exit 1; }

# Build the project in release mode
echo "Building Spacedrive CLI (release mode)..."
cargo build --release || { echo "Error: Cargo build failed."; exit 1; }

# Define source and destination paths
SOURCE_BINARY="$PROJECT_ROOT/target/release/$BINARY_NAME"
DEST_BINARY="$INSTALL_DIR/$BINARY_NAME"

# Ensure the installation directory exists
mkdir -p "$INSTALL_DIR" || { echo "Error: Could not create installation directory: $INSTALL_DIR"; exit 1; }

# Copy the new binary to the installation directory
echo "Copying new binary to $DEST_BINARY..."
cp "$SOURCE_BINARY" "$DEST_BINARY" || { echo "Error: Could not copy binary to $DEST_BINARY. Check permissions."; exit 1; }

echo "Spacedrive CLI updated successfully!"

# Optional: Display the version of the newly installed binary
if [ -x "$DEST_BINARY" ]; then
    echo "Installed version:"
    "$DEST_BINARY" --version
fi
