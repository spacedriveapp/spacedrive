#!/bin/bash

SCRIPT_REPO='https://github.com/strukturag/libheif.git'
SCRIPT_TAG='v1.16.2'

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" libheif

  cd libheif

  mkdir build && cd build

  cmake \
    -GNinja \
    -DCMAKE_TOOLCHAIN_FILE="$FFBUILD_CMAKE_TOOLCHAIN" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="$FFBUILD_PREFIX" \
    -DCMAKE_INSTALL_BINDIR="$FFBUILD_PREFIX"/bin \
    -DCMAKE_INSTALL_LIBDIR="$FFBUILD_PREFIX"/lib \
    -DBUILD_SHARED_LIBS=ON \
    -DBUILD_TESTING=OFF \
    -DWITH_EXAMPLES=OFF \
    -DWITH_FUZZERS=OFF \
    -DWITH_REDUCED_VISIBILITY=ON \
    -DWITH_DEFLATE_HEADER_COMPRESSION=ON \
    -DWITH_AOM_DECODER_PLUGIN=OFF \
    -DWITH_AOM_ENCODER_PLUGIN=OFF \
    -DWITH_DAV1D_PLUGIN=OFF \
    -DWITH_LIBDE265_PLUGIN=OFF \
    -DWITH_RAV1E_PLUGIN=OFF \
    -DWITH_SvtEnc_PLUGIN=OFF \
    -DWITH_X265_PLUGIN=OFF \
    ..

  ninja -j$(nproc)
  ninja install

  mv "$FFBUILD_PREFIX/bin"/*.dll "$FFBUILD_PREFIX/lib"
}
