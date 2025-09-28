#!/bin/bash

# Build script for Spacedrive iOS Core Static Library
# This script builds the Rust core as a static library for iOS

set -e

echo "🔨 Building Spacedrive Core for iOS Static Library..."

# Configuration
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IOS_CORE_DIR="$PROJECT_ROOT/apps/ios/sd-ios-core"
TARGET_DIR="$PROJECT_ROOT/apps/ios/Spacedrive/Frameworks"

# Create frameworks directory
mkdir -p "$TARGET_DIR"

# iOS targets
IOS_TARGETS=(
    "aarch64-apple-ios"      # iOS devices (ARM64)
    "aarch64-apple-ios-sim"  # iOS Simulator (ARM64)
    "x86_64-apple-ios"       # iOS Simulator (x86_64) - for Intel Macs
)

echo "📱 Building for iOS targets: ${IOS_TARGETS[*]}"

# Build for each target
for target in "${IOS_TARGETS[@]}"; do
    echo "🔨 Building for target: $target"

    cd "$IOS_CORE_DIR"

    # Build with mobile feature enabled
    cargo build --release --target "$target" --lib

    echo "✅ Built successfully for $target"
done

echo "🔨 Creating Universal Static Library..."

# Create universal library directory
mkdir -p "$TARGET_DIR/libsd_ios_core"

# Copy static libraries
cp "$PROJECT_ROOT/target/aarch64-apple-ios/release/libsd_ios_core.a" "$TARGET_DIR/libsd_ios_core/libsd_ios_core_device.a"
cp "$PROJECT_ROOT/target/aarch64-apple-ios-sim/release/libsd_ios_core.a" "$TARGET_DIR/libsd_ios_core/libsd_ios_core_sim_arm64.a"
cp "$PROJECT_ROOT/target/x86_64-apple-ios/release/libsd_ios_core.a" "$TARGET_DIR/libsd_ios_core/libsd_ios_core_sim_x86_64.a"

# Create universal library for simulator
cd "$TARGET_DIR/libsd_ios_core"
lipo -create libsd_ios_core_sim_arm64.a libsd_ios_core_sim_x86_64.a -output libsd_ios_core_sim.a

# Create final universal library
lipo -create libsd_ios_core_device.a libsd_ios_core_sim.a -output libsd_ios_core.a

# Clean up intermediate files
rm libsd_ios_core_device.a libsd_ios_core_sim_arm64.a libsd_ios_core_sim_x86_64.a libsd_ios_core_sim.a

echo "✅ Universal static library created!"
echo "📁 Library location: $TARGET_DIR/libsd_ios_core/libsd_ios_core.a"

echo ""
echo "🎉 iOS Core build complete!"
echo ""
echo "Next steps:"
echo "1. Add libsd_ios_core.a to your Xcode project"
echo "2. Add SDIOSCore.h to your Xcode project"
echo "3. Add SDIOSCoreBridge.swift to your Xcode project"
echo "4. Link the static library in Xcode build settings"
echo "5. Test the embedded core integration"
