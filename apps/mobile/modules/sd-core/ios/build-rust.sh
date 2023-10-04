#!/usr/bin/env sh

set -eu

if [ "${CI:-}" = "true" ]; then
  set -x
fi

if [ -z "${HOME:-}" ]; then
  HOME="$(CDPATH='' cd -- "$(osascript -e 'set output to (POSIX path of (path to home folder))')" && pwd)"
  export HOME
fi

echo "Building 'sd-mobile-ios' library..."

__dirname="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"

# Ensure target dir exists
TARGET_DIRECTORY="${__dirname}/../../../../../target"
mkdir -p "$TARGET_DIRECTORY"
TARGET_DIRECTORY="$(CDPATH='' cd -- "$TARGET_DIRECTORY" && pwd)"

if [ "${CONFIGURATION:-}" != "Debug" ]; then
  CARGO_FLAGS=--release
  export CARGO_FLAGS
fi

# TODO: Also do this for non-Apple Silicon Macs
if [ "${SPACEDRIVE_CI:-}" = "1" ]; then
  # Required for CI
  export PATH="${CARGO_HOME:-"${HOME}/.cargo"}/bin:$PATH"

  cargo build -p sd-mobile-ios --target x86_64-apple-ios

  if [ "${PLATFORM_NAME:-}" = "iphonesimulator" ]; then
    lipo -create -output "$TARGET_DIRECTORY"/libsd_mobile_iossim.a "$TARGET_DIRECTORY"/x86_64-apple-ios/debug/libsd_mobile_ios.a
  else
    lipo -create -output "$TARGET_DIRECTORY"/libsd_mobile_ios.a "$TARGET_DIRECTORY"/x86_64-apple-ios/debug/libsd_mobile_ios.a
  fi
  exit 0
fi

if [ "${PLATFORM_NAME:-}" = "iphonesimulator" ]; then
  cargo build -p sd-mobile-ios --target aarch64-apple-ios-sim
  lipo -create -output "$TARGET_DIRECTORY"/libsd_mobile_iossim.a "$TARGET_DIRECTORY"/aarch64-apple-ios-sim/debug/libsd_mobile_ios.a
else
  cargo build -p sd-mobile-ios --target aarch64-apple-ios
  lipo -create -output "$TARGET_DIRECTORY"/libsd_mobile_ios.a "$TARGET_DIRECTORY"/aarch64-apple-ios/debug/libsd_mobile_ios.a
fi
