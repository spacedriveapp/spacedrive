#!/bin/bash

SCRIPT_REPO="https://github.com/ultravideo/kvazaar.git"
SCRIPT_TAG="v2.2.0"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" kvazaar
  cd kvazaar

  ./autogen.sh

  local myconf=(
    --prefix="$FFBUILD_PREFIX"
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

  echo "Cflags.private: -DKVZ_STATIC_LIB" >>"$FFBUILD_PREFIX"/lib/pkgconfig/kvazaar.pc
}
