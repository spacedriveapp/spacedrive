#!/bin/bash

SCRIPT_REPO="https://bitbucket.org/multicoreware/x265_git.git"
SCRIPT_COMMIT="34532bda12a3a3141880582aa186a59cd4538ae6"

ffbuild_dockerbuild() {
  git clone "$SCRIPT_REPO" x265
  cd x265
  git checkout "$SCRIPT_COMMIT"

  local common_config=(
    -DCMAKE_INSTALL_PREFIX="/opt/x265"
    -DCMAKE_TOOLCHAIN_FILE="$FFBUILD_CMAKE_TOOLCHAIN"
    -DCMAKE_BUILD_TYPE=Release
    -DENABLE_CLI=OFF
    -DCMAKE_ASM_NASM_FLAGS=-w-macro-params-legacy
  )

  mkdir 8bit 10bit 12bit
  cmake -S source -B 12bit -G Ninja \
    "${common_config[@]}" \
    -DHIGH_BIT_DEPTH=ON \
    -DEXPORT_C_API=OFF \
    -DENABLE_SHARED=OFF \
    -DENABLE_STATIC=ON \
    -DMAIN12=ON
  ninja -C 12bit -j"$(nproc)"

  cmake -S source -B 10bit -G Ninja \
    "${common_config[@]}" \
    -DENABLE_HDRDENABLE_HDR10_PLUS=ON \
    -DHIGH_BIT_DEPTH=ON \
    -DEXPORT_C_API=OFF \
    -DENABLE_SHARED=OFF \
    -DENABLE_STATIC=ON
  ninja -C 10bit -j"$(nproc)"

  cmake -S source -B 8bit -G Ninja \
    "${common_config[@]}" \
    -DEXTRA_LIB='x265_main10.a;x265_main12.a' \
    -DENABLE_HDRDENABLE_HDR10_PLUS=ON \
    -DEXTRA_LINK_FLAGS=-L. \
    -DENABLE_SHARED=ON \
    -DENABLE_STATIC=OFF \
    -DLINKED_10BIT=ON \
    -DLINKED_12BIT=ON

  ln -s ../10bit/libx265.a 8bit/libx265_main10.a
  ln -s ../12bit/libx265.a 8bit/libx265_main12.a

  ninja -C 8bit -j"$(nproc)"

  ninja -C 8bit install

  echo "Libs.private: -lstdc++" >>/opt/x265/lib/pkgconfig/x265.pc

  cp -nav /opt/x265/* "${FFBUILD_PREFIX}/"
  mkdir -p /opt/dlls/
  cp -nav /opt/x265/* /opt/dlls/
}
