#!/bin/bash

SCRIPT_REPO='https://github.com/FFmpeg/FFmpeg.git'
SCRIPT_BRANCH="release/${FFMPEG_VERSION:-6.0}"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_BRANCH" ffmpeg

  cd ffmpeg

  # Broken configs:
  # --enable-lto (Broken on Windows)

  ./configure \
    --cpu="x86_64" \
    --arch='x86_64' \
    --prefix="/opt/dlls" \
    --target-os=mingw32 \
    --pkg-config=pkg-config \
    --pkg-config-flags="--static" \
    --cross-prefix="$FFBUILD_CROSS_PREFIX" \
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
    --disable-w32threads \
    --disable-xmm-clobber-test \
    --disable-neon-clobber-test \
    --enable-amf \
    --enable-libass \
    --enable-avcodec \
    --enable-avfilter \
    --enable-avformat \
    --enable-bzlib \
    --enable-cuda-llvm \
    --enable-ffnvcodec \
    --enable-libfreetype \
    --enable-libfribidi \
    --enable-gpl \
    --enable-gray \
    --enable-iconv \
    --enable-inline-asm \
    --enable-libdav1d \
    --enable-libjxl \
    --enable-libopenjpeg \
    --enable-libopus \
    --enable-libshaderc \
    --enable-libsoxr \
    --enable-libvorbis \
    --enable-libvpl \
    --enable-libvpx \
    --enable-libwebp \
    --enable-libzimg \
    --enable-libzvbi \
    --enable-lzma \
    --enable-openal \
    --enable-opencl \
    --enable-opengl \
    --enable-optimizations \
    --enable-pic \
    --enable-postproc \
    --enable-pthreads \
    --enable-shared \
    --enable-small \
    --enable-swscale \
    --enable-version3 \
    --enable-vulkan \
    --enable-zlib \
    --enable-cross-compile \
    --extra-cflags='-DLIBTWOLAME_STATIC' \
    --extra-cxxflags='' \
    --extra-ldflags='-pthread' \
    --extra-ldexeflags='' \
    --extra-libs='-lgomp -lstdc++'

  make -j"$(nproc)" V=1

  make PREFIX="/opt/dlls" install
}
