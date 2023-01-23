#! /bin/zsh

set -e

TARGET_DIRECTORY=../../../target

CARGO_FLAGS=
if [[ $CONFIGURATION != "Debug" ]]; then
  CARGO_FLAGS=--release
fi

if [[ $PLATFORM_NAME = "iphonesimulator" ]]
then
    cargo build -p sd-core-ios --target aarch64-apple-ios-sim
    lipo -create -output $TARGET_DIRECTORY/libsd_core_ios-iossim.a $TARGET_DIRECTORY/aarch64-apple-ios-sim/debug/libsd_core_ios.a
else
    cargo build -p sd-core-ios --target aarch64-apple-ios
    lipo -create -output $TARGET_DIRECTORY/libsd_core_ios-ios.a $TARGET_DIRECTORY/aarch64-apple-ios/debug/libsd_core_ios.a
fi
