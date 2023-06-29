#!/bin/bash

SCRIPT_REPO="https://gitlab.freedesktop.org/freetype/freetype.git"
SCRIPT_TAG="VER-2-13-1"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" freetype
  cd freetype

  mkdir build && cd build

  local myconf=(
    --prefix="$FFBUILD_PREFIX"
    --buildtype=release
    --default-library=static
    --cross-file=/cross.meson
    -Dpng=enabled
    -Dbzip2=enabled
    -Dtests=disabled
    -Dbrotli=enabled
    -Dharfbuzz=enabled
  )

  meson "${myconf[@]}" ..
  ninja -j"$(nproc)"
  ninja install
}
