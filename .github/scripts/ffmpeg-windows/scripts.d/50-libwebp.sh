#!/bin/bash

SCRIPT_REPO="https://chromium.googlesource.com/webm/libwebp"
SCRIPT_COMMIT="761f49c3ab1c91b8e911840a4f6f246308b7c242"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_COMMIT" webp
  cd webp

  ./autogen.sh

  local myconf=(
    --prefix="$FFBUILD_PREFIX"
    --disable-shared
    --enable-static
    --with-pic
    --enable-libwebpmux
    --disable-libwebpextras
    --disable-libwebpdemux
    --disable-sdl
    --disable-gl
    --disable-png
    --disable-jpeg
    --disable-tiff
    --disable-gif
  )

  if [[ $TARGET == win* || $TARGET == linux* ]]; then
    myconf+=(
      --host="$FFBUILD_TOOLCHAIN"
    )
  else
    echo "Unknown target"
    return -1
  fi

  export CFLAGS="$CFLAGS -fcommon"

  ./configure "${myconf[@]}"
  make -j"$(nproc)"
  make install
}
