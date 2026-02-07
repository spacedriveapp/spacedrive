#!/bin/bash
set -e

cd "$(dirname "$0")/../../core"

echo "Building Spacedrive Mobile Core for Android..."

# Auto-detect Android NDK if not set
if [ -z "$ANDROID_NDK_ROOT" ]; then
    # Try common locations
    if [ -d "$HOME/Library/Android/sdk/ndk" ]; then
        # macOS Android Studio location - find the latest NDK version
        ANDROID_NDK_ROOT=$(ls -d "$HOME/Library/Android/sdk/ndk"/* 2>/dev/null | sort -V | tail -1)
    elif [ -d "$ANDROID_HOME/ndk" ]; then
        ANDROID_NDK_ROOT=$(ls -d "$ANDROID_HOME/ndk"/* 2>/dev/null | sort -V | tail -1)
    elif [ -d "/usr/local/lib/android/sdk/ndk" ]; then
        # Linux CI location
        ANDROID_NDK_ROOT=$(ls -d "/usr/local/lib/android/sdk/ndk"/* 2>/dev/null | sort -V | tail -1)
    fi

    if [ -n "$ANDROID_NDK_ROOT" ]; then
        echo "Auto-detected ANDROID_NDK_ROOT: $ANDROID_NDK_ROOT"
        export ANDROID_NDK_ROOT
    else
        echo "Error: Could not find Android NDK. Please set ANDROID_NDK_ROOT environment variable."
        exit 1
    fi
fi

# Also set ANDROID_NDK for compatibility
export ANDROID_NDK="$ANDROID_NDK_ROOT"

OUTPUT_DIR="../android/src/main/jniLibs"
mkdir -p "$OUTPUT_DIR"

# Use mobile-dev profile for faster builds (no LTO, parallel codegen)
# See Cargo.toml [profile.mobile-dev] for settings

# Build for arm64-v8a (most modern Android devices)
echo "Building for arm64-v8a..."

# Clear any environment variables that might conflict with cargo-ndk's cross-compilation setup
unset CMAKE_TOOLCHAIN_FILE 2>/dev/null || true
unset CMAKE_TOOLCHAIN_FILE_aarch64_linux_android 2>/dev/null || true
unset CFLAGS 2>/dev/null || true
unset CXXFLAGS 2>/dev/null || true
unset CFLAGS_aarch64_linux_android 2>/dev/null || true

# Don't use NDK toolchain file - let cargo-ndk handle cross-compilation via CC/CXX
# The toolchain file conflicts with cargo-ndk's --target flags
export ANDROID_ABI=arm64-v8a
export ANDROID_PLATFORM=android-24

cargo ndk --platform 24 -t arm64-v8a -o "$OUTPUT_DIR" build --profile mobile-dev

# Optional: Build for armeabi-v7a (older 32-bit devices)
# echo "Building for armeabi-v7a..."
# cargo ndk --platform 24 -t armeabi-v7a -o "$OUTPUT_DIR" build --profile mobile-dev

# Optional: Build for x86_64 (emulators)
# echo "Building for x86_64..."
# cargo ndk --platform 24 -t x86_64 -o "$OUTPUT_DIR" build --profile mobile-dev

echo "Android Rust build complete!"
