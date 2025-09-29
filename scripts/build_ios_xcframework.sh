#!/bin/bash

# The DEFINITIVE iOS build script for Spacedrive
# Builds the v2 Core as an XCFramework for iOS development and distribution
# Replaces all other iOS build scripts

set -e

echo "🔨 Building Spacedrive v2 Core XCFramework for iOS..."

# Configuration
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IOS_CORE_DIR="$PROJECT_ROOT/apps/ios/sd-ios-core"
TARGET_DIR="$PROJECT_ROOT/apps/ios/sd-ios-core"
FRAMEWORK_NAME="sd_ios_core"

# Create build directory
BUILD_DIR="$TARGET_DIR/build"
mkdir -p "$BUILD_DIR"

# iOS targets
IOS_DEVICE_TARGET="aarch64-apple-ios"
IOS_SIM_ARM64_TARGET="aarch64-apple-ios-sim"
IOS_SIM_X86_64_TARGET="x86_64-apple-ios"

echo "📱 Building for iOS targets..."

cd "$IOS_CORE_DIR"

# Build for iOS device (ARM64)
echo "🔨 Building for iOS device (ARM64)..."
IPHONEOS_DEPLOYMENT_TARGET=12.0 cargo build --release --target "$IOS_DEVICE_TARGET"
echo "✅ Device build complete"

# Build for iOS simulator (ARM64) - M1/M2 Macs
echo "🔨 Building for iOS Simulator (ARM64)..."
IPHONEOS_DEPLOYMENT_TARGET=12.0 cargo build --release --target "$IOS_SIM_ARM64_TARGET"
echo "✅ Simulator ARM64 build complete"

# Build for iOS simulator (x86_64) - Intel Macs
echo "🔨 Building for iOS Simulator (x86_64)..."
IPHONEOS_DEPLOYMENT_TARGET=12.0 cargo build --release --target "$IOS_SIM_X86_64_TARGET"
echo "✅ Simulator x86_64 build complete"

echo "🔨 Creating XCFramework..."

# Create framework structure for device
DEVICE_FRAMEWORK_DIR="$BUILD_DIR/${FRAMEWORK_NAME}-device.framework"
mkdir -p "$DEVICE_FRAMEWORK_DIR"
cp "$IOS_CORE_DIR/target/$IOS_DEVICE_TARGET/release/libsd_ios_core.a" "$DEVICE_FRAMEWORK_DIR/$FRAMEWORK_NAME"

# Create framework structure for simulator (universal)
SIM_FRAMEWORK_DIR="$BUILD_DIR/${FRAMEWORK_NAME}-simulator.framework"
mkdir -p "$SIM_FRAMEWORK_DIR"

# Create universal simulator library
lipo -create \
    "$IOS_CORE_DIR/target/$IOS_SIM_ARM64_TARGET/release/libsd_ios_core.a" \
    "$IOS_CORE_DIR/target/$IOS_SIM_X86_64_TARGET/release/libsd_ios_core.a" \
    -output "$SIM_FRAMEWORK_DIR/$FRAMEWORK_NAME"

# Create Info.plist for both frameworks
cat > "$DEVICE_FRAMEWORK_DIR/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>$FRAMEWORK_NAME</string>
    <key>CFBundleIdentifier</key>
    <string>com.spacedrive.core</string>
    <key>CFBundleName</key>
    <string>$FRAMEWORK_NAME</string>
    <key>CFBundlePackageType</key>
    <string>FMWK</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>CFBundleSupportedPlatforms</key>
    <array>
        <string>iPhoneOS</string>
    </array>
    <key>MinimumOSVersion</key>
    <string>12.0</string>
</dict>
</plist>
EOF

cat > "$SIM_FRAMEWORK_DIR/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>$FRAMEWORK_NAME</string>
    <key>CFBundleIdentifier</key>
    <string>com.spacedrive.core</string>
    <key>CFBundleName</key>
    <string>$FRAMEWORK_NAME</string>
    <key>CFBundlePackageType</key>
    <string>FMWK</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>CFBundleSupportedPlatforms</key>
    <array>
        <string>iPhoneSimulator</string>
    </array>
    <key>MinimumOSVersion</key>
    <string>12.0</string>
</dict>
</plist>
EOF

# Update existing XCFramework (which Xcode is already using)
XCFRAMEWORK_PATH="$TARGET_DIR/${FRAMEWORK_NAME}.xcframework"

# Update device framework
cp "$DEVICE_FRAMEWORK_DIR/$FRAMEWORK_NAME" "$XCFRAMEWORK_PATH/ios-arm64/libsd_ios_core.a"

# Update simulator framework
cp "$SIM_FRAMEWORK_DIR/$FRAMEWORK_NAME" "$XCFRAMEWORK_PATH/ios-arm64-simulator/libsd_ios_core.a"

# Clean up build directory
rm -rf "$BUILD_DIR"

echo "✅ XCFramework updated successfully!"
echo "📁 XCFramework location: $XCFRAMEWORK_PATH"
echo "📱 Xcode will automatically use the updated framework"
echo ""
echo "🎉 iOS Core build complete! Ready to test."
