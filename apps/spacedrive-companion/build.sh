#!/bin/bash

# Spacedrive Companion App Build Script

set -e

echo "Building Spacedrive SwiftUI Companion App..."

# Check if we're in the right directory
if [ ! -f "Package.swift" ]; then
    echo "Error: Package.swift not found. Please run this script from the spacedrive-companion directory."
    exit 1
fi

# Build the app
echo "Building with Swift Package Manager..."
swift build -c release

# Check if build was successful
if [ $? -eq 0 ]; then
    echo "✅ Build successful!"
    echo ""
    echo "To run the app:"
    echo "  swift run SpacedriveCompanion"
    echo ""
    echo "Or build and run in one command:"
    echo "  swift run"
else
    echo "❌ Build failed!"
    exit 1
fi
