#!/bin/bash

SCRIPT_REPO="https://github.com/google/brotli.git"
SCRIPT_TAG="v1.0.9"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" brotli
  cd brotli

  mkdir build && cd build

  cmake -G Ninja -DCMAKE_TOOLCHAIN_FILE="$FFBUILD_CMAKE_TOOLCHAIN" -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$FFBUILD_PREFIX" \
    -DCMAKE_POSITION_INDEPENDENT_CODE=ON -DBUILD_SHARED_LIBS=OFF ..
  ninja -j"$(nproc)"
  ninja install
}
