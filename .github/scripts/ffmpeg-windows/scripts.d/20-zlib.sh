#!/bin/bash

SCRIPT_REPO="https://github.com/madler/zlib.git"
SCRIPT_TAG="v1.2.13"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" zlib
  cd zlib

  local myconf=(
    --prefix="$FFBUILD_PREFIX"
    --static
  )

  if [[ $TARGET == win* || $TARGET == linux* ]]; then
    export CC="${FFBUILD_CROSS_PREFIX}gcc"
    export AR="${FFBUILD_CROSS_PREFIX}ar"
  else
    echo "Unknown target"
    return 255
  fi

  ./configure "${myconf[@]}"
  make -j"$(nproc)"
  make install
}
