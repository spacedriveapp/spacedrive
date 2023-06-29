#!/bin/bash

SCRIPT_REPO="https://github.com/fribidi/fribidi.git"
SCRIPT_TAG="v1.0.13"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" fribidi
  cd fribidi

  mkdir build && cd build

  local myconf=(
    --prefix="$FFBUILD_PREFIX"
    --buildtype=release
    --default-library=static
    --cross-file=/cross.meson
    -Dbin=false
    -Ddocs=false
    -Dtests=false
  )

  meson "${myconf[@]}" ..
  ninja -j"$(nproc)"
  ninja install

  sed -i 's/Cflags:/Cflags: -DFRIBIDI_LIB_STATIC/' "$FFBUILD_PREFIX"/lib/pkgconfig/fribidi.pc
}
