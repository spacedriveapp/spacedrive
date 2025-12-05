#!/bin/bash
set -e

echo "🦀 Building Spacedrive Mobile Core..."

cd "$(dirname "$0")/../core"

# Detect platform and build appropriate target
if [ "${PLATFORM_NAME:-}" = "iphonesimulator" ]; then
  echo "📱 Building for iOS Simulator..."
  case "$(uname -m)" in
    "arm64")
      IPHONEOS_DEPLOYMENT_TARGET=18.0 cargo build --target aarch64-apple-ios-sim --release --no-default-features
      TARGET="aarch64-apple-ios-sim"
      LIB_DIR="../ios/libs/simulator"
      ;;
    "x86_64")
      IPHONEOS_DEPLOYMENT_TARGET=18.0 cargo build --target x86_64-apple-ios --release --no-default-features
      TARGET="x86_64-apple-ios"
      LIB_DIR="../ios/libs/simulator"
      ;;
  esac
else
  echo "📱 Building for iOS Device..."
  IPHONEOS_DEPLOYMENT_TARGET=18.0 cargo build --target aarch64-apple-ios --release --no-default-features
  TARGET="aarch64-apple-ios"
  LIB_DIR="../ios/libs/device"
fi

# Create libs directory if it doesn't exist
mkdir -p "$LIB_DIR"

# Copy the built library
echo "📦 Copying library to $LIB_DIR..."
cp "target/$TARGET/release/libsd_mobile_core.a" "$LIB_DIR/"

echo "✅ Spacedrive Mobile Core build complete!"
