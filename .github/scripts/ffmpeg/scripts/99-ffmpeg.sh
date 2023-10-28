#!/usr/bin/env -S bash -euo pipefail

echo "Download ffmpeg..."
mkdir -p ffmpeg

curl -LSs 'https://github.com/FFmpeg/FFmpeg/archive/refs/tags/n6.0.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C ffmpeg

cd ffmpeg

# Patch ffmpeg compat tool for rc.exe to use zig-rc
sed -i 's/rc.exe/zig-rc/' compat/windows/mswindres

echo "Build ffmpeg..."

env_specific_arg=()
case "$TARGET" in
  aarch64-macos* | aarch64-windows*)
    env_specific_arg+=(
      --disable-cuda-llvm
      --disable-ffnvcodec
    )
    ;;
  *)
    env_specific_arg+=(
      --enable-cuda-llvm
      --enable-ffnvcodec
    )
    ;;
esac

case "$TARGET" in
  *windows*)
    # TODO: Add support for pthreads on Windows
    env_specific_arg+=(
      --disable-pthreads
    )
    ;;
  *)
    env_specific_arg+=(
      --enable-pthreads
    )
    ;;
esac

if ! ./configure \
  --cpu="${TARGET%%-*}" \
  --arch="${TARGET%%-*}" \
  --prefix="/opt/out" \
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
  --nm=false \
  --ar=zig-ar \
  --strip=true \
  --cc=zig-cc \
  --cxx=zig-c++ \
  --nvcc=clang \
  --ranlib=zig-ranlib \
  --host-cc=clang \
  --windres="$(pwd)/compat/windows/mswindres" \
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
  --enable-lto \
  --enable-lzma \
  --enable-opencl \
  --enable-optimizations \
  --enable-pic \
  --enable-postproc \
  --enable-shared \
  --enable-small \
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

make PREFIX="/opt/out" install
