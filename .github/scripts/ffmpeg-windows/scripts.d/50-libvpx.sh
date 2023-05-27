#!/bin/bash

SCRIPT_REPO="https://chromium.googlesource.com/webm/libvpx"
SCRIPT_COMMIT="8f14f66490f8bfbbf01ce809629c02631baf5b8f"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_COMMIT" libvpx
  cd libvpx

  local myconf=(
    --disable-shared
    --enable-static
    --enable-pic
    --disable-examples
    --disable-tools
    --disable-docs
    --disable-unit-tests
    --enable-vp9-highbitdepth
    --prefix="$FFBUILD_PREFIX"
  )

  if [[ $TARGET == win64 ]]; then
    myconf+=(
      --target=x86_64-win64-gcc
    )
    export CROSS="$FFBUILD_CROSS_PREFIX"
  elif [[ $TARGET == win32 ]]; then
    myconf+=(
      --target=x86-win32-gcc
    )
    export CROSS="$FFBUILD_CROSS_PREFIX"
  elif [[ $TARGET == linux64 ]]; then
    myconf+=(
      --target=x86_64-linux-gcc
    )
    export CROSS="$FFBUILD_CROSS_PREFIX"
  elif [[ $TARGET == linuxarm64 ]]; then
    myconf+=(
      --target=arm64-linux-gcc
    )
    export CROSS="$FFBUILD_CROSS_PREFIX"
  else
    echo "Unknown target"
    return -1
  fi

  ./configure "${myconf[@]}"
  make -j$(nproc)
  make install

  # Work around strip breaking LTO symbol index
  "$RANLIB" "$FFBUILD_PREFIX"/lib/libvpx.a
}
