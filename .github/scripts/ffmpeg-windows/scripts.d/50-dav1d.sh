#!/bin/bash

SCRIPT_REPO="https://code.videolan.org/videolan/dav1d.git"
SCRIPT_TAG="1.2.1"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" dav1d
  cd dav1d

  mkdir build && cd build

  local myconf=(
    --prefix="$FFBUILD_PREFIX"
    --buildtype=release
    --default-library=shared
  )

  if [[ $TARGET == win* || $TARGET == linux* ]]; then
    myconf+=(
      --cross-file=/cross.meson
    )
  else
    echo "Unknown target"
    return -1
  fi

  meson "${myconf[@]}" ..
  ninja -j"$(nproc)"
  ninja install

  bak_dll
}
