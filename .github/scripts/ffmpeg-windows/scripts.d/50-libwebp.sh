#!/bin/bash

SCRIPT_REPO="https://github.com/webmproject/libwebp.git"
SCRIPT_TAG="1.3.2"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" libwebp
  cd libwebp

  ./autogen.sh

  local myconf=(
    --host="$FFBUILD_TOOLCHAIN"
    --prefix="$FFBUILD_PREFIX"
    --disable-shared
    --enable-static
    --with-pic
    --enable-everything
    --disable-sdl
    --disable-png
    --disable-jpeg
    --disable-tiff
    --disable-gif
  )

  ./configure "${myconf[@]}"
  make -j"$(nproc)"
  make install
}
