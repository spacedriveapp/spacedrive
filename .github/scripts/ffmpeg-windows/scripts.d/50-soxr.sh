#!/bin/bash

SCRIPT_REPO="https://git.code.sf.net/p/soxr/code"
SCRIPT_TAG="0.1.3"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" soxr
  cd soxr

  mkdir build && cd build

  cmake -DCMAKE_TOOLCHAIN_FILE="$FFBUILD_CMAKE_TOOLCHAIN" -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$FFBUILD_PREFIX" -DWITH_OPENMP=ON -DBUILD_TESTS=OFF -DBUILD_EXAMPLES=OFF -DBUILD_SHARED_LIBS=OFF ..
  make -j"$(nproc)"
  make install

  echo "Libs.private: -lgomp" >>"$FFBUILD_PREFIX"/lib/pkgconfig/soxr.pc
}
