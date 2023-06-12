#!/bin/bash

SCRIPT_REPO="https://github.com/KhronosGroup/Vulkan-Headers.git"
SCRIPT_COMMIT="v1.3.252"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_COMMIT" vkheaders
  cd vkheaders

  mkdir build && cd build

  cmake -DCMAKE_TOOLCHAIN_FILE="$FFBUILD_CMAKE_TOOLCHAIN" -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$FFBUILD_PREFIX" ..
  make -j"$(nproc)"
  make install

  cat >"$FFBUILD_PREFIX"/lib/pkgconfig/vulkan.pc <<EOF
prefix=$FFBUILD_PREFIX
includedir=\${prefix}/include

Name: vulkan
Version: ${SCRIPT_COMMIT:1}
Description: Vulkan (Headers Only)
Cflags: -I\${includedir}
EOF
}
