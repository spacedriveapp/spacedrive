#!/bin/bash

SCRIPT_REPO="https://github.com/google/shaderc.git"
SCRIPT_COMMIT="4a8f5e537f20bbcfe4b11f1fe45314f1dcbfddf6"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_COMMIT" shaderc
  cd shaderc

  ./utils/git-sync-deps

  mkdir build && cd build

  cmake -GNinja -DCMAKE_TOOLCHAIN_FILE="$FFBUILD_CMAKE_TOOLCHAIN" -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$FFBUILD_PREFIX" \
    -DSHADERC_SKIP_TESTS=ON -DSHADERC_SKIP_EXAMPLES=ON -DSHADERC_SKIP_COPYRIGHT_CHECK=ON \
    -DENABLE_EXCEPTIONS=ON -DENABLE_CTEST=OFF -DENABLE_GLSLANG_BINARIES=OFF -DSPIRV_SKIP_EXECUTABLES=ON \
    -DSPIRV_TOOLS_BUILD_STATIC=ON -DBUILD_SHARED_LIBS=OFF ..
  ninja -j$(nproc)
  ninja install

  # for some reason, this does not get installed...
  cp libshaderc_util/libshaderc_util.a "$FFBUILD_PREFIX"/lib

  echo "Libs: -lstdc++" >>"$FFBUILD_PREFIX"/lib/pkgconfig/shaderc_combined.pc
  echo "Libs: -lstdc++" >>"$FFBUILD_PREFIX"/lib/pkgconfig/shaderc_static.pc

  cp "$FFBUILD_PREFIX"/lib/pkgconfig/{shaderc_combined,shaderc}.pc

  if [[ $TARGET == win* ]]; then
    rm -r "$FFBUILD_PREFIX"/bin "$FFBUILD_PREFIX"/lib/*.dll.a
  elif [[ $TARGET == linux* ]]; then
    rm -r "$FFBUILD_PREFIX"/bin "$FFBUILD_PREFIX"/lib/*.so*
  else
    echo "Unknown target"
    return -1
  fi
}
