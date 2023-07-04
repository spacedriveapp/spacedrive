#!/usr/bin/env bash

# This script builds ffmpeg for macOS using osxcross.
# This script is heavly influenced by:
#   https://github.com/FFmpeg/FFmpeg/blob/ea3d24bbe3c58b171e55fe2151fc7ffaca3ab3d2/configure
#   https://github.com/GerardSoleCa/macports-ports/blob/6f646dfaeb58ccb4a8b877df1ae4eecc4650fac7/multimedia/ffmpeg-upstream/Portfile
#   https://github.com/arthenica/ffmpeg-kit/blob/47f85fa9ea3f8c34f3c817b87d8667b61b87d0bc/scripts/apple/ffmpeg.sh
#   https://github.com/zimbatm/ffmpeg-static/blob/3206c0d74cd129c2ddfc3e928dcd3ea317d54857/build.sh

set -e          # exit immediate if an error occurs in a pipeline
set -E          # make commands inherit ERR trap
set -u          # don't allow not set variables to be utilized
set -o pipefail # trace ERR through pipes
set -o errtrace # trace ERR through 'time command' and other functions

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
CC="${TRIPLE}-clang"
if ! command -v "$CC" 2>/dev/null; then
  echo "$CC not found" >&2
  exit 1
fi

# Get osxcross root directory
_osxcross_root="$(dirname "$(dirname "$(command -v "$CC")")")"

# Check macports root exists
_macports_root="${_osxcross_root}/macports/pkgs/opt/local"
if ! [ -d "$_macports_root" ]; then
  echo "macports root not found: $_macports_root" >&2
  exit 1
fi
ln -s "$_macports_root" /opt/local

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

setup_cross_env() {
  export CC
  export LD="${TRIPLE}-ld"
  export AR="${TRIPLE}-ar"
  export CXX="${TRIPLE}-clang++"
  export STRIP="${TRIPLE}-strip"
  export CMAKE="${TRIPLE}-cmake"
  export RANLIB="${TRIPLE}-ranlib"
  export PKG_CONFIG="${TRIPLE}-pkg-config"
}

# Change cwd to libwebp source root
CDPATH='' cd -- /srv/libwebp

# Configure libwebp
(
  setup_cross_env
  ./autogen.sh
  ./configure \
    --host="$TRIPLE" \
    --prefix="/opt/local" \
    --disable-shared \
    --enable-static \
    --with-sysroot="${_sdk}" \
    --with-pic \
    --enable-everything \
    --disable-sdl \
    --disable-png \
    --disable-jpeg \
    --disable-tiff \
    --disable-gif

  # Build libwebp
  make -j"$(nproc)" install
)

# Create a tmp TARGET_DIR
TARGET_DIR="$(mktemp -d -t target-XXXXXXXXXX)"

# Change cwd to libheif source root
mkdir -p /srv/libheif/build
CDPATH='' cd -- /srv/libheif/build

# Configure libheif
"${TRIPLE}-cmake" \
  -GNinja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX="${TARGET_DIR}" \
  -DCMAKE_INSTALL_BINDIR="${TARGET_DIR}/bin" \
  -DCMAKE_INSTALL_LIBDIR="${TARGET_DIR}/lib" \
  -DCMAKE_TOOLCHAIN_FILE="${_osxcross_root}/toolchain.cmake" \
  -DLIBSHARPYUV_INCLUDE_DIR="${_macports_root}/include/webp" \
  -DBUILD_TESTING=OFF \
  -DBUILD_SHARED_LIBS=ON \
  -DWITH_DAV1D=ON \
  -DWITH_DAV1D_PLUGIN=OFF \
  -DWITH_LIBDE265=ON \
  -DWITH_LIBDE265_PLUGIN=OFF \
  -DWITH_LIBSHARPYUV=ON \
  -DWITH_FUZZERS=OFF \
  -DWITH_EXAMPLES=OFF \
  -DWITH_UNCOMPRESSED_CODEC=ON \
  -DWITH_REDUCED_VISIBILITY=ON \
  -DWITH_DEFLATE_HEADER_COMPRESSION=ON \
  -DENABLE_PLUGIN_LOADING=OFF \
  -DENABLE_MULTITHREADING_SUPPORT=ON \
  ..

# Build libheif
ninja -j"$(nproc)" install

# Change cwd to ffmpeg source root
CDPATH='' cd -- /srv/ffmpeg

# Save FFmpeg version
FFMPEG_VERSION="$(xargs printf '%s' <VERSION)"

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
  --extra-cflags=-DLIBTWOLAME_STATIC \
  --extra-cxxflags="-xc++-header" \
  --disable-static \
  --disable-debug \
  --disable-doc \
  --disable-htmlpages \
  --disable-txtpages \
  --disable-manpages \
  --disable-podpages \
  --disable-indevs \
  --disable-outdevs \
  --disable-parser=avs2 \
  --disable-parser=avs3 \
  --disable-postproc \
  --disable-programs \
  --disable-libwebp \
  --disable-sdl2 \
  --disable-metal \
  --disable-network \
  --disable-openssl \
  --disable-schannel \
  --disable-securetransport \
  --disable-xlib \
  --disable-libxcb \
  --disable-libxcb-shm \
  --disable-libxcb-xfixes \
  --disable-libxcb-shape \
  --disable-libv4l2 \
  --disable-v4l2-m2m \
  --disable-vulkan \
  --disable-cuda-llvm \
  --disable-w32threads \
  --disable-xmm-clobber-test \
  --disable-neon-clobber-test \
  --enable-appkit \
  --enable-audiotoolbox \
  --enable-avcodec \
  --enable-avfilter \
  --enable-avformat \
  --enable-avfoundation \
  --enable-bzlib \
  --enable-coreimage \
  --enable-cross-compile \
  --enable-gpl \
  --enable-gray \
  --enable-iconv \
  --enable-inline-asm \
  --enable-libdav1d \
  --enable-libjxl \
  --enable-libopenjpeg \
  --enable-libopus \
  --enable-libsoxr \
  --enable-libvorbis \
  --enable-libvpx \
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

# Build FFMpeg
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
    libvorbis/COPYING)
(cd /srv && cp -avt "$_framework_docs" --parents libwebp/COPYING)

# libvorbis, libogg share the same license
ln -s libvorbis "${_framework_docs}/libogg"

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

# Process built libraries to be compatible with the Framework structure
cd "$TARGET_DIR/lib"

# Move all symlinks of built libraries to Framework
while IFS= read -r -d '' _lib; do
  # Copy symlinks to the output directory
  cp -Ppv "$_lib" "/${_framework}/Libraries/${_lib#./}"
  rm "$_lib"
done < <(find . -type l -print0)

# Populate queue with built libraries
set -- # Clear command line arguments
while IFS= read -r -d '' _lib; do
  set -- "$@" "${_lib#./}"
done < <(find . -name '*.dylib' -print0)

while [ $# -gt 0 ]; do
  # Loop through each of the library's dependencies
  for _dep in $("${TRIPLE}-otool" -L "$1" | tail -n+3 | awk '{print $1}'); do
    case "$_dep" in
      # Built libs inter dependency
      "${TARGET_DIR}/lib/"*)
        _linker_path="@loader_path/${_dep#"${TARGET_DIR}/lib/"}"
        ;;
      # Macports dependency (/opt/local/lib means it was installed by Macports)
      "@rpath/"* | /opt/local/lib/*)
        _dep_rel="${_dep#'@rpath/'}"
        _dep_rel="${_dep_rel#/opt/local/lib/}"
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

# Copy all built headers to framework
cp -av "${TARGET_DIR}/include/"* "/${_framework}/Headers/"

# Strip all libraries
"${TRIPLE}-strip" -S "/${_framework}/Libraries/"*.dylib
