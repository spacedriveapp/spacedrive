#!/bin/bash

SCRIPT_REPO="https://github.com/xiph/vorbis.git"
SCRIPT_TAG="v1.3.7"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" vorbis
  cd vorbis

  ./autogen.sh

  local myconf=(
    --host="$FFBUILD_TOOLCHAIN"
    --prefix="$FFBUILD_PREFIX"
    --disable-shared
    --enable-static
    --disable-oggtest
  )

  ./configure "${myconf[@]}"
  make -j"$(nproc)"
  make install
}
