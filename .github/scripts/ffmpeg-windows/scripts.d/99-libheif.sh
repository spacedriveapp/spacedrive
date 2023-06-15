#!/bin/bash

SCRIPT_REPO='https://github.com/strukturag/libheif.git'
SCRIPT_TAG='v1.16.2'

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" libheif

  cd libheif

  mkdir build && cd build

  cmake \
    -GNinja \
    -DCMAKE_GNUtoMS=ON \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX='/opt/dlls' \
    -DCMAKE_INSTALL_BINDIR='/opt/dlls/bin' \
    -DCMAKE_INSTALL_LIBDIR='/opt/dlls/lib' \
    -DCMAKE_TOOLCHAIN_FILE="$FFBUILD_CMAKE_TOOLCHAIN" \
    -DCMAKE_WINDOWS_EXPORT_ALL_SYMBOLS=ON \
    -DBUILD_TESTING=OFF \
    -DBUILD_SHARED_LIBS=ON \
    -DWITH_X265=ON \
    -DWITH_X265_PLUGIN=OFF \
    -DWITH_DAV1D=ON \
    -DWITH_DAV1D_PLUGIN=OFF \
    -DWITH_RAV1E=ON \
    -DWITH_RAV1E_PLUGIN=OFF \
    -DWITH_LIBDE265=ON \
    -DWITH_LIBDE265_PLUGIN=OFF \
    -DWITH_LIBSHARPYUV=ON \
    -DWITH_FUZZERS=OFF \
    -DWITH_EXAMPLES=OFF \
    -DWITH_UNCOMPRESSED_CODEC=ON \
    -DWITH_REDUCED_VISIBILITY=ON \
    -DWITH_DEFLATE_HEADER_COMPRESSION=ON \
    -DENABLE_PLUGIN_LOADING=OFF \
    -DENABLE_MULTITHREADING_SUPPORT=ON \
    ..

  ninja -j"$(nproc)"
  ninja install

  cd /opt/dlls/bin

  "${FFBUILD_CROSS_PREFIX}gendef" libheif.dll

  "${FFBUILD_CROSS_PREFIX}dlltool" -m i386:x86-64 -d libheif.def -l libheif.lib -D libheif.dll
}
