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

no_ext() {
  set -- "$1" "$(basename "$1")"
  while [ "${2%.*}" != "$2" ]; do
    set -- "$1" "${2%.*}"
  done

  printf '%s/%s' "$(dirname "$1")" "$2"
}

# Clear command line arguments
set --
# Populate queue with ffmpeg libraries
while IFS= read -r -d '' _lib; do
  # Remove leading ./
  _lib="${_lib#./}"
  # Add it to the queue to have it's dependencies copied
  set -- "$@" "$_lib"
  # Copy library to the output directory
  cp -p "$_lib" "${OUT_DIR}/${_lib}"
done < <(find . -name '*.dylib' -print0)

# # Copy static library to the output directory
# while IFS= read -r -d '' _lib; do
#   cp -p "$_lib" "${OUT_DIR}/lib/${_lib}"
# done < <(find . -name '*.a' -print0)

while [ $# -gt 0 ]; do
  # Loop through each of the library's dependency
  for _dep in $("${TRIPLE}-otool" -L "$1" | tail -n+2 | awk '{print $1}'); do
    case "$_dep" in
      # dependencies in the ffmpeg directory
      "${TARGET_DIR}/lib/"*)
        _dep_rel="${_dep#"${TARGET_DIR}/lib/"}"
        ;;
      # /opt/local/lib means macports installed it
      /opt/local/lib/*)
        _dep_rel="${_dep#/opt/local/lib/}"
        if [ ! -f "$_dep_rel" ]; then
          # Copy dependency to the current directory if this is the first time we see it
          cp -p -L "${_macports_root}/lib/${_dep_rel}" "./${_dep_rel}"
          # Add it to the queue to have it's own dependencies processed
          set -- "$@" "$_dep_rel"
          # # Copy static verion of dependency to the output directory
          # while IFS= read -r -d '' _static_dep; do
          #   cp -p "${_macports_root}/lib/$_static_dep" "$OUT_DIR/lib/${_static_dep}"
          # done < <(
          #   cd "${_macports_root}/lib/"
          #   find . -wholename "$(no_ext "$_dep_rel")*.a" -print0
          # )
        fi
        ;;
      *) # Ignore system libraries
        continue
        ;;
    esac

    # Change the linked dependency path to use @executable_path/../Frameworks to make it compatible with an .app bundle
    "${TRIPLE}-install_name_tool" -change "$_dep" "@executable_path/../Frameworks/${_dep_rel}" "$1"
  done

  # Update the library's own id to use @executable_path/../Frameworks
  aarch64-apple-darwin21.4-install_name_tool -id "@executable_path/../Frameworks/${1}" "$1"

  # Copy the library to the output directory
  cp -p "$1" "${OUT_DIR}/${1}"
  ln -s "../${1}" "${OUT_DIR}/lib/${1}"

  # Remove library from queue
  shift
done

# Copy all headers to the output directory
cp -r "${_macports_root}/include/"* "$OUT_DIR/include"
cp -r "${TARGET_DIR}/include/"* "$OUT_DIR/include"
