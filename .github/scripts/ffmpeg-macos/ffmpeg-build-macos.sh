#!/usr/bin/env bash

# This script builds ffmpeg for macOS using osxcross.
# This script is heavly influenced by:
#   https://github.com/FFmpeg/FFmpeg/blob/ea3d24bbe3c58b171e55fe2151fc7ffaca3ab3d2/configure
#   https://github.com/GerardSoleCa/macports-ports/blob/6f646dfaeb58ccb4a8b877df1ae4eecc4650fac7/multimedia/ffmpeg-upstream/Portfile
#   https://github.com/arthenica/ffmpeg-kit/blob/47f85fa9ea3f8c34f3c817b87d8667b61b87d0bc/scripts/apple/ffmpeg.sh
#   https://github.com/zimbatm/ffmpeg-static/blob/3206c0d74cd129c2ddfc3e928dcd3ea317d54857/build.sh

set -euox pipefail

if [ "$#" -ne 2 ]; then
  echo "Usage: $0 <target-arch> <macos-version>" >&2
  exit 1
fi

if [ -z "$MACOSX_DEPLOYMENT_TARGET" ]; then
  echo "You must set MACOSX_DEPLOYMENT_TARGET first." >&2
  exit 1
fi

ARCH="$1"
MACOS_VERSION="$2"

set -- # Clear command line arguments
if [ "$ARCH" = "x86_64" ]; then
  TARGET_CPU="x86_64"
  TARGET_ARCH="x86_64"
  set -- --enable-x86asm
elif [ "$ARCH" = "aarch64" ]; then
  TARGET_CPU="armv8"
  TARGET_ARCH="aarch64"
  set -- --enable-neon --enable-asm
else
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

# Gather all SDK libs
_skd_libs="$(
  while IFS= read -r -d '' _lib; do
    _lib="${_lib#"${_sdk}/usr/lib/"}"
    _lib="${_lib%.*}"
    printf '%s.dylib\n' "$_lib"
  done < <(find "${_sdk}/usr/lib" \( -name '*.tbd' -o -name '*.dylib' \) -print0) \
    | sort -u
)"

# Change cwd to the script directory (which should be ffmpeg source root)
CDPATH='' cd -- "$(dirname -- "$0")"

# Save FFmpeg version
FFMPEG_VERSION="$(xargs printf '%s' <VERSION)"

# Create a tmp TARGET_DIR
TARGET_DIR="$(mktemp -d -t ffmpeg-macos-XXXXXXXXXX)"
trap 'rm -rf "$TARGET_DIR"' EXIT

# Configure FFMpeg.
# NOTICE: This isn't autotools
# TODO: Metal suport is disabled because no open source toolchain is available for it
# TODO: Maybe try macOS own metal compiler under darling? https://github.com/darlinghq/darling/issues/326
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
  --sysroot="$_sdk" \
  --cross-prefix="${TRIPLE}-" \
  --ranlib="${TRIPLE}-ranlib" \
  --prefix="${TARGET_DIR}" \
  --arch="${TARGET_ARCH}" \
  --cpu="${TARGET_CPU}" \
  --target-os=darwin \
  --pkg-config="${TRIPLE}-pkg-config" \
  --pkg-config-flags="--static" \
  --extra-ldflags="-Bstatic -headerpad_max_install_names" \
  --extra-ldexeflags="-Bstatic" \
  --extra-cxxflags="-xc++-header" \
  --disable-alsa \
  --disable-cuda \
  --disable-cuvid \
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
  --disable-neon-clobber-test \
  --disable-network \
  --disable-nvdec \
  --disable-nvenc \
  --disable-openssl \
  --disable-outdevs \
  --disable-podpages \
  --disable-postproc \
  --disable-programs \
  --disable-schannel \
  --disable-sdl2 \
  --disable-securetransport \
  --disable-sndio \
  --disable-static \
  --disable-txtpages \
  --disable-v4l2-m2m \
  --disable-vaapi \
  --disable-vdpau \
  --disable-vulkan \
  --disable-xlib \
  --disable-xmm-clobber-test \
  --enable-appkit \
  --enable-audiotoolbox \
  --enable-avcodec \
  --enable-avfilter \
  --enable-avformat \
  --enable-avfoundation \
  --enable-bzlib \
  --enable-coreimage \
  --enable-cross-compile \
  --enable-fontconfig \
  --enable-gpl \
  --enable-iconv \
  --enable-inline-asm \
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
  --enable-libtwolame \
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
  --enable-opengl \
  --enable-optimizations \
  --enable-pic \
  --enable-postproc \
  --enable-pthreads \
  --enable-small \
  --enable-shared \
  --enable-swscale \
  --enable-version3 \
  --enable-videotoolbox \
  --enable-zlib \
  "$@"

make -j"$(nproc)" install

# Create FFMpeg.framework
# https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPFrameworks/Concepts/FrameworkAnatomy.html
# Create the framework basic directory structure
_framework="FFMpeg.framework"
mkdir -p "/${_framework}/Versions/A/"{Headers,Resources,Libraries}

# Copy licenses to Framework
_framework_docs="/${_framework}/Versions/A/Resources/English.lproj/Documentation"
mkdir -p "$_framework_docs"

# FFMpeg license
cp -avt "$_framework_docs" COPYING* LICENSE*

# Dependency licenses which are not covered by FFMpeg licenses
(cd "${_macports_root}/share/doc" \
  && cp -avt "$_framework_docs" --parents \
    zimg/COPYING \
    webp/COPYING \
    libpng/LICENSE \
    libvorbis/COPYING \
    freetype/LICENSE.TXT \
    fontconfig/COPYING)

# libvorbis, libogg and libtheora share the same license
ln -s libvorbis "${_framework_docs}/libogg"
ln -s libvorbis "${_framework_docs}/libtheora"

# Create required framework symlinks
ln -s A "/${_framework}/Versions/Current"
ln -s Versions/Current/Headers "/${_framework}/Headers"
ln -s Versions/Current/Resources "/${_framework}/Resources"
ln -s Versions/Current/Libraries "/${_framework}/Libraries"

# Framework Info.plist (based on macOS internal OpenGL.framework Info.plist)
cat <<EOF >"/${_framework}/Versions/Current/Resources/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>English</string>
    <key>CFBundleExecutable</key>
    <string>FFMpeg</string>
    <key>CFBundleGetInfoString</key>
    <string>FFMpeg ${FFMPEG_VERSION}</string>
    <key>CFBundleIdentifier</key>
    <string>com.spacedrive.ffmpeg</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>FFMpeg</string>
    <key>CFBundlePackageType</key>
    <string>FMWK</string>
    <key>CFBundleShortVersionString</key>
    <string>${FFMPEG_VERSION}</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>CFBundleVersion</key>
    <string>${FFMPEG_VERSION}</string>
</dict>
</plist>
EOF

# Process FFMpeg libraries to be compatible with the Framework structure
cd "$TARGET_DIR/lib"

# Move all symlinks of ffmpeg libraries to Framework
while IFS= read -r -d '' _lib; do
  # Copy symlinks to the output directory
  cp -Ppv "$_lib" "/${_framework}/Libraries/${_lib#./}"
  rm "$_lib"
done < <(find . -type l -print0)

# Populate queue with ffmpeg libraries
set -- # Clear command line arguments
while IFS= read -r -d '' _lib; do
  set -- "$@" "${_lib#./}"
done < <(find . -name '*.dylib' -print0)

# Copy all symlinks of libheif libraries to Framework
while IFS= read -r -d '' _lib; do
  # Copy symlinks to the output directory
  cp -Ppv "$_lib" "/${_framework}/Libraries/${_lib#"${_macports_root}/lib/"}"
done < <(find "${_macports_root}/lib" -type l \( -name 'libheif.*' -a -name '*.dylib' \) -print0)

# Copy libheif to cwd and add it to queue
while IFS= read -r -d '' _lib; do
  _lib_rel="${_lib#"${_macports_root}/lib/"}"
  cp -Lpv "$_lib" "./${_lib_rel}"
  set -- "$@" "${_lib_rel}"
done < <(find "${_macports_root}/lib" -type f \( -name 'libheif.*' -a -name '*.dylib' \) -print0)

while [ $# -gt 0 ]; do
  # Loop through each of the library's dependencies
  for _dep in $("${TRIPLE}-otool" -L "$1" | tail -n+2 | awk '{print $1}'); do
    case "$_dep" in
      # FFMpeg inter dependency
      "${TARGET_DIR}/lib/"*)
        _linker_path="@loader_path/${_dep#"${TARGET_DIR}/lib/"}"
        ;;
      # Macports dependency (/opt/local/lib means it was installed by Macports)
      /opt/local/lib/*)
        _dep_rel="${_dep#/opt/local/lib/}"
        # Check if the macports dependency is already included in the macOS SDK
        if [ -n "$(comm -12 <(printf "%s" "$_dep_rel") <(printf "%s" "$_skd_libs"))" ]; then
          # Relink libs already included in macOS SDK
          _linker_path="/usr/lib/${_dep_rel}"
        else
          _linker_path="@loader_path/${_dep_rel}"
          if ! [ -e "${_macports_root}/lib/${_dep_rel}" ]; then
            echo "Missing macports dependency: ${_dep_rel}"
            exit 1
          elif ! { [ -f "$_dep_rel" ] || [ -e "/${_framework}/Libraries/${_dep_rel}" ]; }; then
            # Copy dependency to the current directory if this is the first time we see it
            cp -Lpv "${_macports_root}/lib/${_dep_rel}" "./${_dep_rel}"
            # Add it to the queue to have it's own dependencies processed
            set -- "$@" "$_dep_rel"
          fi
        fi
        ;;
      *) # Ignore system libraries
        continue
        ;;
    esac

    # Change the dependency linker path to make it compatible with an .app bundle
    "${TRIPLE}-install_name_tool" -change "$_dep" "$_linker_path" "$1"
  done

  # Update the library's own id
  "${TRIPLE}-install_name_tool" -id "@executable_path/../Frameworks/${_framework}/Libraries/${1}" "$1"

  # Copy the library to framework
  cp -Lpv "$1" "/${_framework}/Libraries/${1}"

  # Remove library from queue
  shift
done

# Copy all libheif headers to framework
cp -av "${_macports_root}/include/libheif" "/${_framework}/Headers/"

# Copy all FFMPEG headers to framework
cp -av "${TARGET_DIR}/include/"* "/${_framework}/Headers/"
