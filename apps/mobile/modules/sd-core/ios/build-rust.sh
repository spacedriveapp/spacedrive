#!/usr/bin/env sh

set -eu

if [ "${CI:-}" = "true" ]; then
  set -x
fi

err() {
  for _line in "$@"; do
    echo "$_line" >&2
  done
  exit 1
}

if [ -z "${HOME:-}" ]; then
  HOME="$(CDPATH='' cd -- "$(osascript -e 'set output to (POSIX path of (path to home folder))')" && pwd -P)"
  export HOME
fi

echo "Building 'sd-mobile-ios' library..."

__dirname="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd -P)"

# Ensure target dir exists
TARGET_DIRECTORY="${__dirname}/../../../../../target"
mkdir -p "$TARGET_DIRECTORY"
TARGET_DIRECTORY="$(CDPATH='' cd -- "$TARGET_DIRECTORY" && pwd -P)"

# if [ "${CONFIGURATION:-}" != "Debug" ]; then
#   CARGO_FLAGS=--release
#   export CARGO_FLAGS
# fi

# Required for CI and for everyone I guess?
export PATH="${CARGO_HOME:-"${HOME}/.cargo"}/bin:$PATH"

if [ "${PLATFORM_NAME:-}" = "iphonesimulator" ]; then
  case "$(uname -m)" in
    "arm64" | "aarch64") # M series
      cargo build -p sd-mobile-ios --target aarch64-apple-ios-sim
      lipo -create -output "$TARGET_DIRECTORY"/libsd_mobile_iossim.a "$TARGET_DIRECTORY"/aarch64-apple-ios-sim/debug/libsd_mobile_ios.a
      ;;
    "x86_64") # Intel
      cargo build -p sd-mobile-ios --target x86_64-apple-ios
      lipo -create -output "$TARGET_DIRECTORY"/libsd_mobile_iossim.a "$TARGET_DIRECTORY"/x86_64-apple-ios/debug/libsd_mobile_ios.a
      ;;
    *)
      err 'Unsupported architecture.'
      ;;
  esac
else
  cargo build -p sd-mobile-ios --target aarch64-apple-ios --release
  lipo -create -output "$TARGET_DIRECTORY"/libsd_mobile_ios.a "$TARGET_DIRECTORY"/aarch64-apple-ios/release/libsd_mobile_ios.a
fi
