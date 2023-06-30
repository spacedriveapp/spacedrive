#!/bin/bash

SCRIPT_REPO="https://github.com/xiph/ogg.git"
SCRIPT_TAG="v1.3.5"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" ogg
  cd ogg

  ./autogen.sh

  local myconf=(
    --host="$FFBUILD_TOOLCHAIN"
    --prefix="$FFBUILD_PREFIX"
    --disable-shared
    --enable-static
    --with-pic
  )

  ./configure "${myconf[@]}"
  make -j"$(nproc)"
  make install
}
