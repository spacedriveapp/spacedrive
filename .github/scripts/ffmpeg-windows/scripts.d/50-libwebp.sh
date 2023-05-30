#!/bin/bash

SCRIPT_REPO="https://chromium.googlesource.com/webm/libwebp"
SCRIPT_COMMIT="761f49c3ab1c91b8e911840a4f6f246308b7c242"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_COMMIT" webp
  cd webp

  mkdir build && cd build

  cmake .. -G Ninja \
    -DCMAKE_INSTALL_PREFIX="$FFBUILD_PREFIX" \
    -DCMAKE_TOOLCHAIN_FILE="$FFBUILD_CMAKE_TOOLCHAIN" \
    -DCMAKE_BUILD_TYPE=Release \
    -DENABLE_SHARED=ON \
    -DENABLE_STATIC=OFF \
    -DWEBP_BUILD_ANIM_UTILS=OFF \
    -DWEBP_BUILD_CWEBP=OFF \
    -DWEBP_BUILD_DWEBP=OFF \
    -DWEBP_BUILD_GIF2WEBP=OFF \
    -DWEBP_BUILD_IMG2WEBP=OFF \
    -DWEBP_BUILD_VWEBP=OFF \
    -DWEBP_BUILD_WEBPINFO=OFF \
    -DWEBP_BUILD_WEBPMUX=OFF \
    -DWEBP_BUILD_EXTRAS=OFF \
    -DWEBP_BUILD_WEBP_JS=OFF

  ninja -j"$(nproc)"

  ninja install

  mv "$FFBUILD_PREFIX/bin"/*.dll "$FFBUILD_PREFIX/lib"
}
