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

symlink_libs() {
  if [ $# -ne 2 ]; then
    err "Invalid number of arguments. Usage: symlink_libs <dir1> <dir2>"
  fi

  if [ ! -d "$1" ]; then
    err "Directory '$1' does not exist."
  fi

  if [ ! -d "$2" ]; then
    err "Directory '$2' does not exist."
  fi

  find "$1" -type f -name '*.a' -exec ln -sf "{}" "$2" \;
}

if [ -z "${HOME:-}" ]; then
  HOME="$(CDPATH='' cd -- "$(osascript -e 'set output to (POSIX path of (path to home folder))')" && pwd -P)"
  export HOME
fi

echo "Building 'sd-mobile-ios' library..."

__dirname="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd -P)"
DEPS="${__dirname}/../../../.deps/"
DEPS="$(CDPATH='' cd -- "$DEPS" && pwd -P)"
CARGO_CONFIG="${__dirname}/../../../../../.cargo"
CARGO_CONFIG="$(CDPATH='' cd -- "$CARGO_CONFIG" && pwd -P)/config.toml"

# Ensure target dir exists
TARGET_DIRECTORY="${__dirname}/../../../../../target"
mkdir -p "$TARGET_DIRECTORY"
TARGET_DIRECTORY="$(CDPATH='' cd -- "$TARGET_DIRECTORY" && pwd -P)"

TARGET_CONFIG=debug
# if [ "${CONFIGURATION:-}" = "Release" ]; then
#   set -- --release
#   TARGET_CONFIG=release
# fi

trap 'if [ -e "${CARGO_CONFIG}.bak" ]; then mv "${CARGO_CONFIG}.bak" "$CARGO_CONFIG"; fi' EXIT

# Required for `cargo` to correctly compile the library
RUST_PATH="${CARGO_HOME:-"${HOME}/.cargo"}/bin:$(brew --prefix)/bin:$(env -i /bin/bash --noprofile --norc -c 'echo $PATH')"
if [ "${PLATFORM_NAME:-}" = "iphonesimulator" ]; then
  case "$(uname -m)" in
  "arm64" | "aarch64") # M series
    sed -i.bak "s|FFMPEG_DIR = { force = true, value = \".*\" }|FFMPEG_DIR = { force = true, value = \"${DEPS}/aarch64-apple-ios-sim\" }|" "$CARGO_CONFIG"
    env CARGO_FEATURE_STATIC=1 PATH="$RUST_PATH" cargo build -p sd-mobile-ios --target aarch64-apple-ios-sim "$@"
    lipo -create -output "$TARGET_DIRECTORY"/libsd_mobile_iossim.a "${TARGET_DIRECTORY}/aarch64-apple-ios-sim/${TARGET_CONFIG}/libsd_mobile_ios.a"
    symlink_libs "${DEPS}/aarch64-apple-ios-sim/lib" "$TARGET_DIRECTORY"
    ;;
  "x86_64") # Intel
    sed -i.bak "s|FFMPEG_DIR = { force = true, value = \".*\" }|FFMPEG_DIR = { force = true, value = \"${DEPS}/x86_64-apple-ios\" }|" "$CARGO_CONFIG"
    env CARGO_FEATURE_STATIC=1 PATH="$RUST_PATH" cargo build -p sd-mobile-ios --target x86_64-apple-ios "$@"
    lipo -create -output "$TARGET_DIRECTORY"/libsd_mobile_iossim.a "${TARGET_DIRECTORY}/x86_64-apple-ios/${TARGET_CONFIG}/libsd_mobile_ios.a"
    symlink_libs "${DEPS}/x86_64-apple-ios/lib" "$TARGET_DIRECTORY"
    ;;
  *)
    err 'Unsupported architecture.'
    ;;
  esac
else
  sed -i.bak "s|FFMPEG_DIR = { force = true, value = \".*\" }|FFMPEG_DIR = { force = true, value = \"${DEPS}/aarch64-apple-ios\" }|" "$CARGO_CONFIG"
  env CARGO_FEATURE_STATIC=1 PATH="$RUST_PATH" cargo build -p sd-mobile-ios --target aarch64-apple-ios "$@"
  lipo -create -output "$TARGET_DIRECTORY"/libsd_mobile_ios.a "${TARGET_DIRECTORY}/aarch64-apple-ios/${TARGET_CONFIG}/libsd_mobile_ios.a"
  symlink_libs "${DEPS}/aarch64-apple-ios/lib" "$TARGET_DIRECTORY"
fi
