#!/bin/bash

SCRIPT_REPO="https://github.com/glennrp/libpng.git"
SCRIPT_TAG="v1.6.40"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" libpng
  cd libpng

  local myconf=(
    --host="$FFBUILD_TOOLCHAIN"
    --prefix="$FFBUILD_PREFIX"
    --disable-shared
    --enable-static
    --enable-pic
    --enable-hardware-optimizations
  )

  export CPPFLAGS="-I${FFBUILD_PREFIX}/include"
  export LDFLAGS="-L${FFBUILD_PREFIX}/lib"

  ./configure "${myconf[@]}"
  make -j"$(nproc)"
  make install
}
