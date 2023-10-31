#!/usr/bin/env -S bash -euo pipefail

echo "Download ffmpeg..."
mkdir -p ffmpeg

curl_tar 'https://github.com/FFmpeg/FFmpeg/archive/refs/tags/n6.0.tar.gz' ffmpeg 1

# Backup source
bak_src 'ffmpeg'

cd ffmpeg

echo "Build ffmpeg..."

export CFLAGS="${CFLAGS:-} ${SHARED_FLAGS:-}"
export CXXFLAGS="${CXXFLAGS:-} ${SHARED_FLAGS:-}"

env_specific_arg=()

if [ "$(uname -m)" = "${TARGET%%-*}" ] && (case "$TARGET" in *linux* | x86_64-windows*) exit 0 ;; *) exit 1 ;; esac) then
  # zig cc doesn't support compiling cuda code yet, so we use the host clang for it
  # Unfortunatly that means we only suport cuda in the same architecture as the host system
  # https://github.com/ziglang/zig/pull/10704#issuecomment-1023616464
  env_specific_arg+=(
    --nvcc="clang-17 -target ${TARGET}"
    --enable-cuda-llvm
    --enable-ffnvcodec
    --disable-cuda-nvcc
  )
else
  # There are no drivers for Nvidia GPU for macOS or Windows on ARM
  env_specific_arg+=(
    --nvcc=false
    --disable-cuda-llvm
    --disable-ffnvcodec
  )
fi

case "$TARGET" in
  *macos*)
    env_specific_arg+=(
      --enable-lto
      --enable-pthreads
    )
    ;;
  *linux*)
    env_specific_arg+=(
      --enable-lto
      --enable-pthreads
    )
    ;;
  *windows*)
    # TODO: Add support for pthreads on Windows (zig doesn't ship pthreads-w32 from mingw64)
    # TODO: Add support for mediafoundation on Windows (zig doesn't seem to have the necessary bindings to it yet)
    # TODO: LTO isn't work on Windows rn
    env_specific_arg+=(
      --disable-pthreads
      --disable-mediafoundation
    )
    ;;
esac

if ! ./configure \
  --cpu="${TARGET%%-*}" \
  --arch="${TARGET%%-*}" \
  --prefix="$OUT" \
  --target-os="$(
    case "$TARGET" in
      *linux*)
        echo "linux"
        ;;
      *darwin*)
        echo "darwin"
        ;;
      *windows*)
        echo "mingw64"
        ;;
    esac
  )" \
  --cc=zig-cc \
  --nm=llvm-nm-17 \
  --ar=ar \
  --cxx=zig-c++ \
  --strip=llvm-strip-17 \
  --ranlib=ranlib \
  --host-cc=clang-17 \
  --windres="windres" \
  --x86asmexe=nasm \
  --pkg-config=pkg-config \
  --pkg-config-flags="--static" \
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
  --disable-opengl \
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
  --disable-w32threads \
  --disable-xmm-clobber-test \
  --disable-neon-clobber-test \
  --enable-amf \
  --enable-avcodec \
  --enable-avfilter \
  --enable-avformat \
  --enable-bzlib \
  --enable-cross-compile \
  --enable-gpl \
  --enable-inline-asm \
  --enable-libdav1d \
  --enable-libmp3lame \
  --enable-libopus \
  --enable-libplacebo \
  --enable-libshaderc \
  --enable-libsoxr \
  --enable-libsvtav1 \
  --enable-libtheora \
  --enable-libvorbis \
  --enable-libvpl \
  --enable-libvpx \
  --enable-libx264 \
  --enable-libx265 \
  --enable-libzimg \
  --enable-lzma \
  --enable-opencl \
  --enable-optimizations \
  --enable-postproc \
  --enable-shared \
  --enable-swscale \
  --enable-version3 \
  --enable-vulkan \
  --enable-zlib \
  "${env_specific_arg[@]}"; then
  cat ffbuild/config.log >&2
  exit 1
fi

case "$TARGET" in
  *linux*)
    # Replace incorrect identifyed sysctl as enabled on linux
    sed -i 's/#define HAVE_SYSCTL 1/#define HAVE_SYSCTL 0/' config.h
    ;;
esac

make -j"$(nproc)" V=1

make PREFIX="$OUT" install
