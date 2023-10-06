#!/bin/bash

SCRIPT_REPO="https://github.com/madler/zlib.git"
SCRIPT_TAG="v1.3"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" zlib
  cd zlib

  local myconf=(
    --prefix="$FFBUILD_PREFIX"
    --static
  )

  export CC="${FFBUILD_CROSS_PREFIX}gcc"
  export AR="${FFBUILD_CROSS_PREFIX}ar"

  ./configure "${myconf[@]}"
  make -j"$(nproc)"
  make install
}
