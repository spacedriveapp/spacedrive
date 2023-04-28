#!/usr/bin/env bash

set -euox pipefail

if [ "$#" -ne 4 ]; then
  echo "Usage: $0 <target-arch> <darwin-version> <macos-version> <out-dir>" >&2
  exit 1
fi

ARCH="$1"
DARWIN_VERSION="$2"
MACOS_VERSION="$3"
OUT_DIR="$4"
TRIPLE="${ARCH}-apple-${DARWIN_VERSION}"

# Clear command line arguments
set --
if [ "$ARCH" = "x86_64" ]; then
  set -- --enable-x86asm
elif [ "$ARCH" = "aarch64" ]; then
  set -- --disable-fft
else
  echo "Unsupported architecture: $ARCH" >&2
  exit 1
fi

# Check macOS gcc exists
if ! CC="$(command -v "${TRIPLE}-clang" 2>/dev/null)"; then
  echo "${TRIPLE}-clang not found" >&2
  exit 1
fi
export CC

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

# Change cwd to the script directory (which should be ffmpeg source directory)
cd "$(dirname "$0")"

# Create OUT_DIR if it doesn't exists
mkdir -p "$OUT_DIR"

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
  --extra-cflags="-I${_sdk}/usr/include -I${_osxcross_root}/include -I${_macports_root}/include" \
  --extra-ldflags="-L${_sdk}/usr/lib -L${_osxcross_root}/lib -L${_macports_root}/lib -lSystem" \
  --extra-cxxflags="-xc++-header -I${_sdk}/usr/include -I${_osxcross_root}/include -I${_macports_root}/include" \
  --disable-avdevice \
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
  --disable-protocols \
  --disable-sdl2 \
  --disable-swresample \
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
  # loop through each library dependency
  for _library in $("${TRIPLE}-otool" -L "$1" | awk '{print $1}'); do
    case "$_library" in
      /opt/local/*) # check if the dependency is in /opt/local (which mean it was installed by macports)
        _filename=$(basename "$_library")

        # copy the dependency to the current directory if this is the first time we see it,
        # then add it to the queue to have it's dependencies processed
        if [ ! -f "$_filename" ]; then
          cp "${_macports_root}/${_filename}" ./
          set -- "$@" "$_filename"
        fi

        # change the linked dependency path to use @executable_path/../Frameworks (make it compatible with an .app bundle)
        "${TRIPLE}-install_name_tool" -change "$_library" "@executable_path/../Frameworks/$_filename" "$1"
        ;;
      *) # System library are ignored
        ;;
    esac
  done

  # Copy the library to the output directory
  cp "$1" "$OUT_DIR"

  # Remove library from queue
  shift
done
