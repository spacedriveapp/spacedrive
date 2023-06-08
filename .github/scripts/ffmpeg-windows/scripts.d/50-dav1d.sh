#!/bin/bash

SCRIPT_REPO="https://code.videolan.org/videolan/dav1d.git"
SCRIPT_TAG="1.2.1"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" dav1d
  cd dav1d

  mkdir build && cd build

  local myconf=(
    -Denable_docs=false
    -Denable_tools=false
    -Denable_tests=false
    -Denable_examples=false
    --prefix=/opt/dav1d
    --buildtype=release
    --default-library=shared
  )

  if [[ $TARGET == win* || $TARGET == linux* ]]; then
    myconf+=(
      --cross-file=/cross.meson
    )
  else
    echo "Unknown target"
    return 255
  fi

  meson "${myconf[@]}" ..
  ninja -j"$(nproc)"
  ninja install

  sed -i "s@^prefix=/opt/dav1d\$@prefix=${FFBUILD_PREFIX}@" /opt/dav1d/lib/pkgconfig/dav1d.pc
  cp -nav /opt/dav1d/* "${FFBUILD_PREFIX}/"

  mkdir -p /opt/dlls/
  cp -nav /opt/dav1d/* /opt/dlls/
  rm -r /opt/dlls/lib/pkgconfig
}
