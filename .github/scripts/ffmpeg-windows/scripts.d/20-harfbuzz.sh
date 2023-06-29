#!/bin/bash

SCRIPT_REPO="https://github.com/harfbuzz/harfbuzz.git"
SCRIPT_TAG="7.3.0"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" harfbuzz
  cd harfbuzz

  mkdir build && cd build

  local myconf=(
    --prefix="$FFBUILD_PREFIX"
    --buildtype=release
    --default-library=static
    --cross-file=/cross.meson
    --force-fallback-for=freetype2
    -Dgdi=enabled
    -Dfreetype=enabled
    -Ddirectwrite=enabled
    -Dcoretext=disabled
    -Dtests=disabled
    -Dintrospection=disabled
    -Ddocs=disabled
    -Dutilities=disabled
  )

  meson "${myconf[@]}" ..
  ninja -j"$(nproc)"
  ninja install
}
