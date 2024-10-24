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
  case "$(uname)" in
  "Darwin")
    HOME="$(CDPATH='' cd -- "$(osascript -e 'set output to (POSIX path of (path to home folder))')" && pwd -P)"
    ;;
  "Linux")
    HOME="$(CDPATH='' cd -- "$(getent passwd "$(id -un)" | cut -d: -f6)" && pwd -P)"
    ;;
  *)
    err "Your OS ($(uname)) is not supported by this script." \
      'We would welcome a PR or some help adding your OS to this script.' \
      'https://github.com/spacedriveapp/spacedrive/issues'
    ;;
  esac

  export HOME
fi

echo "Building 'sd-mobile-android' library..."

__dirname="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd -P)"

# Ensure output dir exists
OUTPUT_DIRECTORY="${__dirname}/../../../../../apps/mobile/android/app/src/main/jniLibs"
mkdir -p "$OUTPUT_DIRECTORY"

# Required for CI and for everyone I guess?
export PATH="${CARGO_HOME:-"${HOME}/.cargo"}/bin:$PATH"

# Set the targets to build
# If CI, then we build x86_64 else we build all targets
if [ "${CI:-}" = "true" ]; then
  # TODO: This need to be adjusted for future mobile release CI
  case "$(uname -m)" in
  "arm64" | "aarch64")
    ANDROID_BUILD_TARGET_LIST="arm64-v8a"
    ;;
  "x86_64")
    ANDROID_BUILD_TARGET_LIST="x86_64"
    ;;
  *)
    err 'Unsupported architecture for CI build.'
    ;;
  esac
else
  # ANDROID_BUILD_TARGET_LIST="arm64-v8a armeabi-v7a x86_64"
  ANDROID_BUILD_TARGET_LIST="arm64-v8a"
fi

# Configure build targets CLI arg for `cargo ndk`
echo "Building targets: $ANDROID_BUILD_TARGET_LIST"
set --
for _target in $ANDROID_BUILD_TARGET_LIST; do
  set -- "$@" -t "$_target"
done

cd "${__dirname}/crate"
cargo ndk --platform 34 "$@" -o "$OUTPUT_DIRECTORY" build
# \ --release
