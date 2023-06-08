#!/bin/bash

SCRIPT_REPO="https://github.com/libjxl/libjxl.git"
SCRIPT_COMMIT="7263ec97397a8113cfa4012bf84ca4618198df3b"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_COMMIT" jxl
  cd jxl
  git submodule update --init --recursive --depth 1 --recommend-shallow third_party/{highway,skcms}

  mkdir build && cd build

  if [[ $TARGET == linux* ]]; then
    # our glibc is too old(<2.25), and their detection fails for some reason
    export CXXFLAGS="$CXXFLAGS -DVQSORT_GETRANDOM=0 -DVQSORT_SECURE_SEED=0"
  elif [[ $TARGET == win* ]]; then
    # Fix AVX2 related crash due to unaligned stack memory
    export CXXFLAGS="$CXXFLAGS -Wa,-muse-unaligned-vector-move"
    export CFLAGS="$CFLAGS -Wa,-muse-unaligned-vector-move"
  fi

  cmake -G Ninja \
    -DCMAKE_INSTALL_PREFIX="$FFBUILD_PREFIX" \
    -DCMAKE_TOOLCHAIN_FILE="$FFBUILD_CMAKE_TOOLCHAIN" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
    -DBUILD_SHARED_LIBS=OFF \
    -DJPEGXL_STATIC=OFF \
    -DJPEGXL_ENABLE_TOOLS=OFF \
    -DJPEGXL_ENABLE_VIEWERS=OFF \
    -DJPEGXL_EMSCRIPTEN=OFF \
    -DJPEGXL_ENABLE_DOXYGEN=OFF \
    -DBUILD_TESTING=OFF \
    -DJPEGXL_ENABLE_EXAMPLES=OFF \
    -DJPEGXL_ENABLE_MANPAGES=OFF \
    -DJPEGXL_ENABLE_JNI=OFF \
    -DJPEGXL_ENABLE_PLUGINS=OFF \
    -DJPEGXL_ENABLE_DEVTOOLS=OFF \
    -DJPEGXL_ENABLE_BENCHMARK=OFF \
    -DJPEGXL_BUNDLE_LIBPNG=OFF \
    -DJPEGXL_ENABLE_SJPEG=OFF \
    -DJPEGXL_FORCE_SYSTEM_BROTLI=ON \
    ..
  ninja -j"$(nproc)"
  ninja install

  echo "Cflags.private: -DJXL_STATIC_DEFINE=1" >>"${FFBUILD_PREFIX}"/lib/pkgconfig/libjxl.pc
  echo "Libs.private: -lstdc++" >>"${FFBUILD_PREFIX}"/lib/pkgconfig/libjxl.pc

  echo "Cflags.private: -DJXL_STATIC_DEFINE=1" >>"${FFBUILD_PREFIX}"/lib/pkgconfig/libjxl_threads.pc
  echo "Libs.private: -lstdc++" >>"${FFBUILD_PREFIX}"/lib/pkgconfig/libjxl_threads.pc

  if [[ $TARGET == win* ]]; then
    echo "Libs.private: -ladvapi32" >>"${FFBUILD_PREFIX}"/lib/pkgconfig/libjxl.pc
    echo "Libs.private: -ladvapi32" >>"${FFBUILD_PREFIX}"/lib/pkgconfig/libjxl_threads.pc
  fi
}
