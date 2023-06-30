#!/bin/bash

SCRIPT_REPO="https://github.com/xiph/opus.git"
SCRIPT_TAG="v1.4"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" opus
  cd opus

  ./autogen.sh

  local myconf=(
    --host="$FFBUILD_TOOLCHAIN"
    --prefix="$FFBUILD_PREFIX"
    --disable-shared
    --enable-static
    --disable-extra-programs
  )

  ./configure "${myconf[@]}"
  make -j"$(nproc)"
  make install
}
