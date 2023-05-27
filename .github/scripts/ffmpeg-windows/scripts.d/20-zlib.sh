#!/bin/bash

SCRIPT_REPO="https://github.com/madler/zlib.git"
SCRIPT_COMMIT="04f42ceca40f73e2978b50e93806c2a18c1281fc"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_COMMIT" zlib
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
    return -1
  fi

  ./configure "${myconf[@]}"
  make -j$(nproc)
  make install
}
