#!/bin/bash

SCRIPT_REPO="https://aomedia.googlesource.com/aom"
SCRIPT_TAG="v3.6.1"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" aom
  cd aom

  for patch in /patches/*.patch; do
    echo "Applying $patch"
    git am <"$patch"
  done

  mkdir cmbuild && cd cmbuild

  # Workaround broken build system
  export CFLAGS="$CFLAGS -pthread -I/opt/ffbuild/include/libvmaf"

  cmake \
    -DCMAKE_TOOLCHAIN_FILE="$FFBUILD_CMAKE_TOOLCHAIN" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="$FFBUILD_PREFIX" \
    -DBUILD_SHARED_LIBS=YES \
    -DBUILD_STATIC_LIBS=NO \
    -DENABLE_EXAMPLES=NO \
    -DENABLE_TESTS=NO \
    -DENABLE_TOOLS=NO \
    -DCONFIG_TUNE_VMAF=1 \
    ..
  make -j$(nproc)
  make install

  mv "$FFBUILD_PREFIX/bin"/*.dll "$FFBUILD_PREFIX/lib"

  echo "Requires.private: libvmaf" >>"$FFBUILD_PREFIX/lib/pkgconfig/aom.pc"
}
