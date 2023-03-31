#! /bin/zsh

set -e

TARGET_DIRECTORY=../../../target

CARGO_FLAGS=
if [[ $CONFIGURATION != "Debug" ]]; then
  CARGO_FLAGS=--release
fi

# TODO: Also do this for non-Apple Silicon Macs
if [[ $SPACEDRIVE_CI == "1" ]]; then
  # IF SPACEDRIVE_CI is "1", build for x86_64-apple-ios 
  export PATH="$HOME/.cargo/bin:$PATH"
  export PROTOC=/usr/local/bin/protoc
  if [[ $PLATFORM_NAME = "iphonesimulator" ]]
  then
    cargo build -p sd-mobile-ios --target x86_64-apple-ios
    lipo -create -output $TARGET_DIRECTORY/libsd_mobile_ios-iossim.a $TARGET_DIRECTORY/x86_64-apple-ios/debug/libsd_mobile_ios.a
  else
    cargo build -p sd-mobile-ios --target x86_64-apple-ios
    lipo -create -output $TARGET_DIRECTORY/libsd_mobile_ios-ios.a $TARGET_DIRECTORY/x86_64-apple-ios/debug/libsd_mobile_ios.a
  fi
  exit 0
fi

export PROTOC=/opt/homebrew/bin/protoc

if [[ $PLATFORM_NAME = "iphonesimulator" ]]
then
    cargo build -p sd-mobile-ios --target aarch64-apple-ios-sim
    lipo -create -output $TARGET_DIRECTORY/libsd_mobile_ios-iossim.a $TARGET_DIRECTORY/aarch64-apple-ios-sim/debug/libsd_mobile_ios.a
else
    cargo build -p sd-mobile-ios --target aarch64-apple-ios
    lipo -create -output $TARGET_DIRECTORY/libsd_mobile_ios-ios.a $TARGET_DIRECTORY/aarch64-apple-ios/debug/libsd_mobile_ios.a
fi
