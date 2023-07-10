#!/bin/bash

SCRIPT_REPO="https://github.com/google/brotli.git"
SCRIPT_COMMIT="50ebce107f5b1eb36760c7ec2d4726ec56784373"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_COMMIT" brotli
  cd brotli

  mkdir build && cd build

  cmake -G Ninja \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_TOOLCHAIN_FILE="$FFBUILD_CMAKE_TOOLCHAIN" \
    -DCMAKE_INSTALL_PREFIX="$FFBUILD_PREFIX" \
    -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
    -DBUILD_SHARED_LIBS=OFF \
    -DBROTLI_DISABLE_TESTS=ON \
    -DBROTLI_BUNDLED_MODE=OFF \
    ..

  ninja -j"$(nproc)"
  ninja install
}
