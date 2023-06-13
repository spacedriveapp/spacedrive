#!/bin/bash

SCRIPT_REPO="https://github.com/strukturag/libde265.git"
SCRIPT_TAG="v1.0.11"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" libde265
  cd libde265

  autoreconf -i

  local myconf=(
    --prefix="$FFBUILD_PREFIX"
    --disable-shared
    --enable-static
    --disable-dec265
    --disable-sherlock265
  )

  if [[ $TARGET == win* || $TARGET == linux* ]]; then
    myconf+=(
      --host="$FFBUILD_TOOLCHAIN"
    )
  else
    echo "Unknown target"
    return 255
  fi

  ./configure "${myconf[@]}"
  make -j"$(nproc)"
  make install
}
