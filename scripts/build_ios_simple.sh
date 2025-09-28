#!/bin/bash

# Simple iOS build script for the embedded core
set -e

echo "Building Rust core for iOS..."

# Build for iOS simulator ARM64
echo "Building for iOS simulator ARM64..."
cargo build --target aarch64-apple-ios-sim --release --manifest-path apps/ios/sd-ios-core/Cargo.toml

# Build for iOS device ARM64
echo "Building for iOS device ARM64..."
cargo build --target aarch64-apple-ios --release --manifest-path apps/ios/sd-ios-core/Cargo.toml

# Create universal library
echo "Creating universal library..."
mkdir -p apps/ios/sd-ios-core/target/universal-ios

lipo -create \
    apps/ios/sd-ios-core/target/aarch64-apple-ios-sim/release/libsd_ios_core.a \
    apps/ios/sd-ios-core/target/aarch64-apple-ios/release/libsd_ios_core.a \
    -output apps/ios/sd-ios-core/target/universal-ios/libsd_ios_core.a

echo "✅ Universal library created at: apps/ios/sd-ios-core/target/universal-ios/libsd_ios_core.a"
echo "📱 You can now add this library to your Xcode project"
