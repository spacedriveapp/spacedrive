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

# if [ "${CONFIGURATION:-}" != "Debug" ]; then
#   CARGO_FLAGS=--debug
#   export CARGO_FLAGS
# fi

# Clean OUTPUT_DIRECTORY before building
echo "Cleaning $OUTPUT_DIRECTORY"
rm -rf $OUTPUT_DIRECTORY/*
mkdir -p $OUTPUT_DIRECTORY

# Required for CI and for everyone I guess?
export PATH="${CARGO_HOME:-"${HOME}/.cargo"}/bin:$PATH"

ANDROID_BUILD_TARGET_LIST="arm64-v8a armeabi-v7a x86 x86_64"

# Loop through the list of targets and build them concurrently
cd crate/

# Parallel build
echo "Building targets: $ANDROID_BUILD_TARGET_LIST"

# Build for each target
for target in $ANDROID_BUILD_TARGET_LIST; do
  echo "Building for target: $target"
  cargo ndk --platform 34 -t $target -o $TARGET_DIRECTORY build --release
done

# Clean up apps/mobile/android/app/src/main/jniLibs directory before moving new files
# rm -rf ${__dirname}/apps/mobile/android/app/src/main/jniLibs/*

# Move contents of target directory to apps/mobile/android/app/src/main/jniLibs
echo "Moving files to $OUTPUT_DIRECTORY"
for target in $ANDROID_BUILD_TARGET_LIST; do
  mv $TARGET_DIRECTORY/$target $OUTPUT_DIRECTORY
done
