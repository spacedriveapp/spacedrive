#!/bin/bash

SCRIPT_REPO="https://github.com/strukturag/libde265.git"
SCRIPT_TAG="v1.0.12"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" libde265
  cd libde265

  autoreconf -i

  local myconf=(
    --host="$FFBUILD_TOOLCHAIN"
    --prefix="$FFBUILD_PREFIX"
    --disable-shared
    --enable-static
    --disable-dec265
    --disable-sherlock265
  )

  ./configure "${myconf[@]}"
  make -j"$(nproc)"
  make install
}
