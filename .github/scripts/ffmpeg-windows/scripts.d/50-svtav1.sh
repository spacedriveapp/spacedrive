#!/bin/bash

SCRIPT_REPO="https://gitlab.com/AOMediaCodec/SVT-AV1.git"
SCRIPT_COMMIT="fca45816bb70e431b2e664374d93c39afecb2783"

ffbuild_dockerbuild() {
  git clone "$SCRIPT_REPO" svtav1
  cd svtav1
  git checkout "$SCRIPT_COMMIT"

  mkdir build && cd build

  cmake -DCMAKE_TOOLCHAIN_FILE="$FFBUILD_CMAKE_TOOLCHAIN" -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$FFBUILD_PREFIX" -DBUILD_SHARED_LIBS=OFF -DBUILD_TESTING=OFF -DBUILD_APPS=OFF -DENABLE_AVX512=ON ..
  make -j$(nproc)
  make install
}
