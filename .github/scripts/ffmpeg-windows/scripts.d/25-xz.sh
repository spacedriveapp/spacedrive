#!/bin/bash

SCRIPT_REPO="https://github.com/tukaani-project/xz.git"
SCRIPT_TAG="v5.4.3"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" xz
  cd xz

  ./autogen.sh --no-po4a --no-doxygen

  local myconf=(
    --host="$FFBUILD_TOOLCHAIN"
    --prefix="$FFBUILD_PREFIX"
    --enable-small
    --disable-xz
    --disable-xzdec
    --disable-lzmadec
    --disable-lzmainfo
    --disable-lzma-links
    --disable-scripts
    --disable-doc
    --disable-symbol-versions
    --disable-shared
    --enable-static
    --with-pic
  )

  ./configure "${myconf[@]}"
  make -j"$(nproc)"
  make install
}
