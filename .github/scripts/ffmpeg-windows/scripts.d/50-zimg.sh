#!/bin/bash

SCRIPT_REPO="https://github.com/sekrit-twc/zimg.git"
SCRIPT_COMMIT="332aaac5e99de46ddd5663092779742ec1958b11"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_COMMIT" zimg
  cd zimg
  git submodule update --init --recursive --depth=1

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
