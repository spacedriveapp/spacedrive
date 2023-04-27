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

if [ "$ARCH" != "x86_64" ] && [ "$ARCH" != "aarch64" ]; then
  echo "Unsupported architecture: $ARCH" >&2
  exit 1
fi

# check clang can run
if ! CLANG="$(command -v "${TRIPLE}-clang" 2>/dev/null)"; then
  echo "clang not found" >&2
  exit 1
fi

_osxcross_root="$(dirname "$(dirname "$CLANG")")"
_macports_root="${_osxcross_root}/macports/pkgs/opt/local"

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
mkdir -p "$OUT_DIR"

TARGET_DIR="$(pwd)/target"

# NOTE: Only gnu linker is supported
./configure \
  --nm="${TRIPLE}-nm" \
  --ar="${TRIPLE}-ar" \
  --as="${TRIPLE}-as" \
  --cc="$CLANG" \
  --cxx="${TRIPLE}-clang++" \
  --arch="${ARCH}" \
  --objcc="$CLANG" \
  --strip="${TRIPLE}-strip" \
  --ranlib="${TRIPLE}-ranlib" \
  --prefix="${TARGET_DIR}" \
  --target="${TRIPLE}" \
  --target-os='darwin' \
  --pkg-config="${TRIPLE}-pkg-config" \
  --extra-cflags="-I${_osxcross_root}/include -I${_macports_root}/include" \
  --extra-ldflags="-L${_sdk}/usr/lib -L${_osxcross_root}/lib -L${_macports_root}/lib -lSystem" \
  --extra-cxxflags="-I${_osxcross_root}/include -I${_macports_root}/include" \
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
  --enable-cross-compile

make -j"$(nproc)" install

cd "$TARGET_DIR/lib"

# Copy all symlinks to dylibs to the output directory
find . -type l -exec cp -t "$OUT_DIR" '{}' +

# Clear command line arguments
set --

# Populate queue with ffmpeg dylibs
for _lib in *.dylib; do
  set -- "$@" "$_lib"
done

while [ $# -gt 0 ]; do
  # loop through each library dependency
  for _library in $("${TRIPLE}-otool" -L "$1" | awk '{print $1}'); do
    case "$_library" in
      /opt/local/*) # check if the library is in /opt/local (which mean it was installed by macports)
        _filename=$(basename "$_library")

        # copy the library to the current directory if this is the first time we see it
        # and add it to the queue
        if [ ! -f "$_filename" ]; then
          cp "${_macports_root}/${_filename}" ./
          set -- "$@" "$_filename"
        fi

        # change the dependency linked path to use @executable_path/../Frameworks
        "${TRIPLE}-install_name_tool" -change "$_library" "@executable_path/../Frameworks/$_filename" "$1"
        ;;
      *) # System library are ignored
        ;;
    esac
  done

  # Copy the library to the output directory
  cp "$1" "$OUT_DIR"

  shift
done
