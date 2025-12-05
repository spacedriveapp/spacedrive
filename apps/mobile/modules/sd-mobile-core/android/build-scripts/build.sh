#!/bin/bash
set -e

cd "$(dirname "$0")/../../core"

echo "Building Spacedrive Mobile Core for Android..."

OUTPUT_DIR="../android/src/main/jniLibs"
mkdir -p "$OUTPUT_DIR"

# Build for arm64-v8a (most modern Android devices)
echo "Building for arm64-v8a..."
cargo ndk --platform 24 -t arm64-v8a -o "$OUTPUT_DIR" build --release

# Optional: Build for armeabi-v7a (older 32-bit devices)
# echo "Building for armeabi-v7a..."
# cargo ndk --platform 24 -t armeabi-v7a -o "$OUTPUT_DIR" build --release

# Optional: Build for x86_64 (emulators)
# echo "Building for x86_64..."
# cargo ndk --platform 24 -t x86_64 -o "$OUTPUT_DIR" build --release

echo "Android Rust build complete!"
