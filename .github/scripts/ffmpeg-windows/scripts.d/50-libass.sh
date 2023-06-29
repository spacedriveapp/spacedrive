#!/bin/bash

SCRIPT_REPO="https://github.com/libass/libass.git"
SCRIPT_TAG="0.17.1"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" libass
  cd libass

  ./autogen.sh

  local myconf=(
    --host="$FFBUILD_TOOLCHAIN"
    --prefix="$FFBUILD_PREFIX"
    --disable-shared
    --enable-static
    --with-pic
    --enable-large-tiles
  )

  export CFLAGS="$CFLAGS -Dread_file=libass_internal_read_file"

  ./configure "${myconf[@]}"
  make -j"$(nproc)"
  make install
}
