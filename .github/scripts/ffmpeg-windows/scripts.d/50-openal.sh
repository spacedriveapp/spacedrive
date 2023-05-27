#!/bin/bash

SCRIPT_REPO="https://github.com/kcat/openal-soft.git"
SCRIPT_COMMIT="d9fed51aa6391debc31dbbca550f055c980afe70"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_COMMIT" openal
  cd openal

  mkdir cm_build && cd cm_build

  cmake -DCMAKE_TOOLCHAIN_FILE="$FFBUILD_CMAKE_TOOLCHAIN" -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$FFBUILD_PREFIX" \
    -DLIBTYPE=STATIC -DALSOFT_UTILS=OFF -DALSOFT_EXAMPLES=OFF ..
  make -j$(nproc)
  make install

  echo "Libs.private: -lstdc++" >>"$FFBUILD_PREFIX"/lib/pkgconfig/openal.pc

  if [[ $TARGET == win* ]]; then
    echo "Libs.private: -lole32" >>"$FFBUILD_PREFIX"/lib/pkgconfig/openal.pc
  fi
}
