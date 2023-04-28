#!/usr/bin/env bash

set -euox pipefail

if [ "$#" -ne 3 ]; then
  echo "Usage: $0 <target-arch> <macos-version> <out-dir>" >&2
  exit 1
fi

ARCH="$1"
MACOS_VERSION="$2"
OUT_DIR="$3"

# Clear command line arguments
set --
if [ "$ARCH" = "x86_64" ]; then
  set -- --enable-x86asm
elif ! [ "$ARCH" = "aarch64" ]; then
  echo "Unsupported architecture: $ARCH" >&2
  exit 1
fi

# Get darwin version and build compiler triple
DARWIN_VERSION="$(basename "$(realpath "$(command -v "oa64-clang")")" | awk -F- '{print $3}')"
TRIPLE="${ARCH}-apple-${DARWIN_VERSION}"

# Check macOS clang exists
if ! CC="$(command -v "${TRIPLE}-clang" 2>/dev/null)"; then
  echo "${TRIPLE}-clang not found" >&2
  exit 1
fi
export CC

# Get osxcross root directory
_osxcross_root="$(dirname "$(dirname "$CC")")"

# Check macports root exists
_macports_root="${_osxcross_root}/macports/pkgs/opt/local"
if ! [ -d "$_macports_root" ]; then
  echo "macports root not found: $_macports_root" >&2
  exit 1
fi

# Check SDK exists
_sdk="${_osxcross_root}/SDK/MacOSX${MACOS_VERSION}.sdk"
if ! [ -d "$_sdk" ]; then
  echo "Invalid MacOS version: $MACOS_VERSION" >&2
  exit 1
fi

# Check that OUT_DIR is a directory or doesn't exists
if [ -e "$OUT_DIR" ] && ! [ -d "$OUT_DIR" ]; then
  echo "Invalid output directory: $OUT_DIR" >&2
  exit 1
fi

# Change cwd to the script directory (which should be ffmpeg source root)
cd "$(dirname "$0")"

# Create OUT_DIR if it doesn't exists
mkdir -p "$OUT_DIR/lib" "$OUT_DIR/include"

# Create a tmp TARGET_DIR
TARGET_DIR="$(mktemp -d -t ffmpeg-macos-XXXXXXXXXX)"
trap 'rm -rf "$TARGET_DIR"' EXIT

# This isn't autotools
./configure \
  --nm="${TRIPLE}-nm" \
  --ar="${TRIPLE}-ar" \
  --as="$CC" \
  --ld="$CC" \
  --cc="$CC" \
  --cxx="${TRIPLE}-clang++" \
  --arch="${ARCH}" \
  --objcc="$CC" \
  --strip="${TRIPLE}-strip" \
  --dep-cc="$CC" \
  --ranlib="${TRIPLE}-ranlib" \
  --prefix="${TARGET_DIR}" \
  --target-os='darwin' \
  --pkg-config="${TRIPLE}-pkg-config" \
  --extra-ldflags="-headerpad_max_install_names" \
  --extra-cxxflags="-xc++-header" \
  --disable-debug \
  --disable-doc \
  --disable-htmlpages \
  --disable-indevs \
  --disable-libjack \
  --disable-libopencore-amrnb \
  --disable-libopencore-amrwb \
  --disable-libpulse \
  --disable-libxcb \
  --disable-libxcb-shape \
  --disable-libxcb-shm \
  --disable-libxcb-xfixes \
  --disable-manpages \
  --disable-metal \
  --disable-network \
  --disable-nvdec \
  --disable-nvenc \
  --disable-outdevs \
  --disable-podpages \
  --disable-programs \
  --disable-protocols \
  --disable-sdl2 \
  --disable-txtpages \
  --disable-vulkan \
  --disable-xlib \
  --enable-audiotoolbox \
  --enable-avcodec \
  --enable-avfilter \
  --enable-avformat \
  --enable-fontconfig \
  --enable-gpl \
  --enable-libaom \
  --enable-libfreetype \
  --enable-libfribidi \
  --enable-libgsm \
  --enable-libmp3lame \
  --enable-libopenjpeg \
  --enable-libopus \
  --enable-librav1e \
  --enable-libsoxr \
  --enable-libsvtav1 \
  --enable-libtheora \
  --enable-libvorbis \
  --enable-libvpx \
  --enable-libwebp \
  --enable-libx264 \
  --enable-libx265 \
  --enable-libxvid \
  --enable-libzimg \
  --enable-lto \
  --enable-lzma \
  --enable-opencl \
  --enable-pic \
  --enable-postproc \
  --enable-pthreads \
  --enable-shared \
  --enable-swscale \
  --enable-version3 \
  --enable-videotoolbox \
  --enable-zlib \
  --enable-cross-compile \
  "$@"

make -j"$(nproc)" install

cd "$TARGET_DIR/lib"

# Move all symlinks to ffmpeg libraries to the output directory
find . -type l -exec mv -t "$OUT_DIR" '{}' +

# Clear command line arguments
set --
# Populate queue with ffmpeg libraries
for _lib in *.dylib; do
  set -- "$@" "$_lib"
done

while [ $# -gt 0 ]; do
  # Loop through each of the library's dependency
  for _dep in $("${TRIPLE}-otool" -L "$1" | tail -n+2 | awk '{print $1}'); do
    _dep_name="$(basename "$_dep")"
    case "$_dep" in
      # dependencies in the same directory
      "${TARGET_DIR}/lib/${_dep_name}")
        _dep_path="${TARGET_DIR}/lib/${_dep_name}"
        ;;
        # /opt/local/lib means macports installed it
      "/opt/local/lib/${_dep_name}")
        if [ ! -f "$_dep_name" ]; then
          # Copy dependency to the current directory if this is the first time we see it
          cp -L "${_macports_root}/lib/${_dep_name}" ./
          # Add it to the queue to have it's own dependencies processed
          set -- "$@" "$_dep_name"
        fi
        _dep_path="/opt/local/lib/${_dep_name}"
        ;;
      *)
        continue
        ;;
    esac

    # Change the linked dependency path to use @executable_path/../Frameworks to make it compatible with an .app bundle
    "${TRIPLE}-install_name_tool" -change "$_dep_path" "@executable_path/../Frameworks/$_dep_name" "$1"
  done

  # Update the library's own id to use @executable_path/../Frameworks
  aarch64-apple-darwin21.4-install_name_tool -id "@executable_path/../Frameworks/${1}" "$1"

  # Copy the library to the output directory
  cp "$1" "$OUT_DIR"
  ln -s "../$1" "$OUT_DIR/lib/$1"

  # Remove library from queue
  shift
done

# Copy all headers to the output directory
cp -r "${_macports_root}/include/"* "$OUT_DIR/include"
cp -r "${TARGET_DIR}/include/"* "$OUT_DIR/include"
