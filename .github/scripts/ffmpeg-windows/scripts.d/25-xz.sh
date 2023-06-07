#!/bin/bash

SCRIPT_REPO="https://github.com/xz-mirror/xz.git"
SCRIPT_TAG="v5.4.3"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" xz
  cd xz

  ./autogen.sh --no-po4a --no-doxygen

  local myconf=(
    --prefix="$FFBUILD_PREFIX"
    --disable-symbol-versions
    --disable-shared
    --enable-static
    --with-pic
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
