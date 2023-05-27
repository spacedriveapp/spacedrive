#!/bin/bash

SCRIPT_REPO='https://github.com/FFmpeg/FFmpeg.git'
SCRIPT_BRANCH='release/6.0'

ffbuild_enabled() {
  return 0
}

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_BRANCH" ffmpeg

  cd ffmpeg

  ./configure \
    --cpu="x86_64" \
    --arch='x86_64' \
    --prefix="$FFBUILD_PREFIX" \
    --target-os=mingw32 \
    --pkg-config=pkg-config \
    --pkg-config-flags="--static" \
    --cross-prefix="$FFBUILD_CROSS_PREFIX" \
    --disable-alsa \
    --disable-debug \
    --disable-doc \
    --disable-indevs \
    --disable-libplacebo \
    --disable-neon-clobber-test \
    --disable-network \
    --disable-outdevs \
    --disable-postproc \
    --disable-programs \
    --disable-schannel \
    --disable-static \
    --disable-v4l2-m2m \
    --disable-vaapi \
    --disable-vdpau \
    --disable-w32threads \
    --disable-xmm-clobber-test \
    --enable-amf \
    --enable-avcodec \
    --enable-avfilter \
    --enable-avformat \
    --enable-bzlib \
    --enable-cuda-llvm \
    --enable-ffnvcodec \
    --enable-gpl \
    --enable-gray \
    --enable-inline-asm \
    --enable-libaom \
    --enable-libdav1d \
    --enable-libjxl \
    --enable-libkvazaar \
    --enable-libmp3lame \
    --enable-libopenjpeg \
    --enable-libopus \
    --enable-librav1e \
    --enable-libshaderc \
    --enable-libsoxr \
    --enable-libsvtav1 \
    --enable-libtheora \
    --enable-libtwolame \
    --enable-libvmaf \
    --enable-libvorbis \
    --enable-libvpl \
    --enable-libvpx \
    --enable-libwebp \
    --enable-libx264 \
    --enable-libx265 \
    --enable-libxvid \
    --enable-libzimg \
    --enable-lzma \
    --enable-openal \
    --enable-opencl \
    --enable-opengl \
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
    --extra-libs='-lgomp'

  make -j"$(nproc)" V=1

  make PREFIX="$FFBUILD_PREFIX" install

  mv "$FFBUILD_PREFIX/bin"/*.dll "$FFBUILD_PREFIX/lib"
}
