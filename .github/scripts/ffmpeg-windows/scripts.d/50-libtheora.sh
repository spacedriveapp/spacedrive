#!/bin/bash

SCRIPT_REPO="https://github.com/xiph/theora.git"
SCRIPT_COMMIT="7180717276af1ebc7da15c83162d6c5d6203aabf"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_COMMIT" theora
  cd theora

  ./autogen.sh

  local myconf=(
    --prefix="$FFBUILD_PREFIX"
    --disable-shared
    --enable-static
    --with-pic
    --disable-examples
    --disable-oggtest
    --disable-vorbistest
    --disable-spec
    --disable-doc
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
