#!/bin/bash

ARTIFACT_URL='https://sourceforge.net/projects/soxr/files/soxr-0.1.3-Source.tar.xz'

ffbuild_dockerbuild() {
  retry-tool sh -c "rm -rf soxr && mkdir -p soxr && curl -LSs '${ARTIFACT_URL}' | tar -xJf- --strip-components=1 -C soxr"
  cd soxr

  mkdir build && cd build

  cmake -DCMAKE_TOOLCHAIN_FILE="$FFBUILD_CMAKE_TOOLCHAIN" -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$FFBUILD_PREFIX" -DWITH_OPENMP=ON -DBUILD_TESTS=OFF -DBUILD_EXAMPLES=OFF -DBUILD_SHARED_LIBS=OFF ..
  make -j"$(nproc)"
  make install

  echo "Libs.private: -lgomp" >>"$FFBUILD_PREFIX"/lib/pkgconfig/soxr.pc
}
