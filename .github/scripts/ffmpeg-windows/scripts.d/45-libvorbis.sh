#!/bin/bash

SCRIPT_REPO="https://github.com/xiph/vorbis.git"
SCRIPT_TAG="v1.3.7"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" vorbis
  cd vorbis

  ./autogen.sh

  local myconf=(
    --prefix="$FFBUILD_PREFIX"
    --disable-shared
    --enable-static
    --disable-oggtest
  )

  if [[ $TARGET == win* || $TARGET == linux* ]]; then
    myconf+=(
      --host="$FFBUILD_TOOLCHAIN"
    )
  else
    echo "Unknown target"
    return -1
  fi

  ./configure "${myconf[@]}"
  make -j"$(nproc)"
  make install
}
