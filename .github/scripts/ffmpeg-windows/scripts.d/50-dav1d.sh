#!/bin/bash

SCRIPT_REPO="https://code.videolan.org/videolan/dav1d.git"
SCRIPT_COMMIT="0b809a9281e0a97441728ad763779e7874209d37"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_COMMIT" dav1d
  cd dav1d

  mkdir build && cd build

  local myconf=(
    --prefix="$FFBUILD_PREFIX"
    --buildtype=release
    --default-library=static
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
  ninja -j$(nproc)
  ninja install
}
