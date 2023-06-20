#!/bin/bash

SCRIPT_REPO="https://github.com/sekrit-twc/zimg.git"
SCRIPT_COMMIT="332aaac5e99de46ddd5663092779742ec1958b11"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_COMMIT" zimg
  cd zimg
  git submodule update --init --recursive --depth=1

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
    return 255
  fi

  ./configure "${myconf[@]}"
  make -j"$(nproc)"
  make install
}
