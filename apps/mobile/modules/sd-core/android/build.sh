#!/usr/bin/env sh

set -eu

if [ "${CI:-}" = "true" ]; then
  set -x
fi

if [ -z "${HOME:-}" ]; then
  HOME="$(CDPATH='' cd -- "$(osascript -e 'set output to (POSIX path of (path to home folder))')" && pwd -P)"
  export HOME
fi

echo "Building 'sd-mobile-android' library..."

__dirname="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd -P)"

# Ensure target dir exists
TARGET_DIRECTORY="${__dirname}/../../../../../target/android"
OUTPUT_DIRECTORY="${__dirname}/../../../../../apps/mobile/android/app/src/main/jniLibs"
mkdir -p "$TARGET_DIRECTORY"
TARGET_DIRECTORY="$(CDPATH='' cd -- "$TARGET_DIRECTORY" && pwd -P)"

# If CI, then we can skip the clean: we know we're starting from a clean state
if [ "${CI:-}" = "true" ]; then
  echo "CI environment detected, skipping clean"
else
  # Clean the target directory
  echo "Cleaning $TARGET_DIRECTORY"
  rm -rf $TARGET_DIRECTORY/*
fi

# Required for CI and for everyone I guess?
export PATH="${CARGO_HOME:-"${HOME}/.cargo"}/bin:$PATH"

# Set the targets to build
# If CI, then we build x86_64 else we build all targets
if [ "${CI:-}" = "true" ]; then
  ANDROID_BUILD_TARGET_LIST="x86_64"
else
  ANDROID_BUILD_TARGET_LIST="arm64-v8a armeabi-v7a x86"
fi

# Loop through the list of targets and build them concurrently
cd crate/

echo "Building targets: $ANDROID_BUILD_TARGET_LIST"

# Build for each target
for target in $ANDROID_BUILD_TARGET_LIST; do
  echo "Building for target: $target"
  cargo ndk --platform 34 -t $target -o $TARGET_DIRECTORY build --release
done

# Move contents of target directory to apps/mobile/android/app/src/main/jniLibs
echo "Moving files to $OUTPUT_DIRECTORY"
for target in $ANDROID_BUILD_TARGET_LIST; do
  mv $TARGET_DIRECTORY/$target $OUTPUT_DIRECTORY
done
