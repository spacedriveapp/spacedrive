#!/bin/bash

SCRIPT_REPO="https://github.com/mirror/x264.git"
SCRIPT_COMMIT="eaa68fad9e5d201d42fde51665f2d137ae96baf0"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_COMMIT" x264
  cd x264

  local myconf=(
    --disable-cli
    --enable-static
    --enable-pic
    --disable-lavf
    --disable-swscale
    --prefix="$FFBUILD_PREFIX"
  )

  if [[ $TARGET == win* || $TARGET == linux* ]]; then
    myconf+=(
      --host="$FFBUILD_TOOLCHAIN"
      --cross-prefix="$FFBUILD_CROSS_PREFIX"
    )
  else
    echo "Unknown target"
    return -1
  fi

  ./configure "${myconf[@]}"
  make -j$(nproc)
  make install
}
