#!/bin/bash

# Build script for Spacedrive iOS Core
# This script builds the Rust core as a static library for iOS

set -e

echo "🔨 Building Spacedrive Core for iOS..."

# Configuration
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CORE_DIR="$PROJECT_ROOT/core"
IOS_DIR="$PROJECT_ROOT/apps/ios"
TARGET_DIR="$IOS_DIR/Spacedrive/Frameworks"

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

    cd "$CORE_DIR"

    # Build with mobile feature enabled
    cargo build --release --target "$target" --features mobile

    echo "✅ Built successfully for $target"
done

echo "🔨 Creating XCFramework..."

# Create XCFramework
cd "$TARGET_DIR"

# Create framework directories
mkdir -p "SpacedriveCore.framework/Headers"
mkdir -p "SpacedriveCore.framework/Modules"

# Copy static libraries
cp "$PROJECT_ROOT/target/aarch64-apple-ios/release/libsd_core.a" "SpacedriveCore.framework/SpacedriveCore"
cp "$PROJECT_ROOT/target/aarch64-apple-ios-sim/release/libsd_core.a" "SpacedriveCore.framework/SpacedriveCore-simulator"
cp "$PROJECT_ROOT/target/x86_64-apple-ios/release/libsd_core.a" "SpacedriveCore.framework/SpacedriveCore-x86_64"

# Create Info.plist
cat > "SpacedriveCore.framework/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleExecutable</key>
    <string>SpacedriveCore</string>
    <key>CFBundleIdentifier</key>
    <string>com.spacedrive.core</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>SpacedriveCore</string>
    <key>CFBundlePackageType</key>
    <string>FMWK</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>MinimumOSVersion</key>
    <string>15.0</string>
</dict>
</plist>
EOF

# Create module.modulemap
cat > "SpacedriveCore.framework/Modules/module.modulemap" << EOF
framework module SpacedriveCore {
    umbrella header "SpacedriveCore.h"
    export *
    module * { export * }
}
EOF

# Create umbrella header
cat > "SpacedriveCore.framework/Headers/SpacedriveCore.h" << EOF
#import <Foundation/Foundation.h>

//! Project version number for SpacedriveCore.
FOUNDATION_EXPORT double SpacedriveCoreVersionNumber;

//! Project version string for SpacedriveCore.
FOUNDATION_EXPORT const unsigned char SpacedriveCoreVersionString[];

// In this header, you should import all the public headers of your framework using statements like #import <SpacedriveCore/PublicHeader.h>

// Mobile-specific exports
extern void handle_core_msg(const char* msg, void (*callback)(const char*));
extern void spawn_core_event_listener(void (*callback)(const char*));
extern void initialize_core(void);
extern void shutdown_core(void);
EOF

echo "✅ XCFramework created successfully!"
echo "📁 Framework location: $TARGET_DIR/SpacedriveCore.framework"

echo ""
echo "🎉 iOS Core build complete!"
echo ""
echo "Next steps:"
echo "1. Add SpacedriveCore.framework to your Xcode project"
echo "2. Add SpacedriveClient package dependency"
echo "3. Update EmbeddedCoreManager to use real core"
