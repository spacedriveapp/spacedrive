#!/bin/bash

SCRIPT_REPO="https://github.com/KhronosGroup/OpenCL-Headers.git"
SCRIPT_REPO2="https://github.com/KhronosGroup/OpenCL-ICD-Loader.git"
SCRIPT_TAG="v2023.04.17"

ffbuild_dockerbuild() {
  mkdir opencl && cd opencl

  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" headers
  mkdir -p "$FFBUILD_PREFIX"/include/CL
  cp -r headers/CL/* "$FFBUILD_PREFIX"/include/CL/.

  git-mini-clone "$SCRIPT_REPO2" "$SCRIPT_TAG" loader
  cd loader

  mkdir build && cd build

  cmake -DCMAKE_TOOLCHAIN_FILE="$FFBUILD_CMAKE_TOOLCHAIN" -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$FFBUILD_PREFIX" \
    -DOPENCL_ICD_LOADER_HEADERS_DIR="$FFBUILD_PREFIX"/include -DOPENCL_ICD_LOADER_BUILD_SHARED_LIBS=OFF \
    -DOPENCL_ICD_LOADER_DISABLE_OPENCLON12=ON -DOPENCL_ICD_LOADER_PIC=ON \
    -DOPENCL_ICD_LOADER_BUILD_TESTING=OFF -DBUILD_TESTING=OFF ..
  make -j"$(nproc)"
  make install

  echo "prefix=$FFBUILD_PREFIX" >OpenCL.pc
  echo "exec_prefix=\${prefix}" >>OpenCL.pc
  echo "libdir=\${exec_prefix}/lib" >>OpenCL.pc
  echo "includedir=\${prefix}/include" >>OpenCL.pc
  echo >>OpenCL.pc
  echo "Name: OpenCL" >>OpenCL.pc
  echo "Description: OpenCL ICD Loader" >>OpenCL.pc
  echo "Version: 9999" >>OpenCL.pc
  echo "Cflags: -I\${includedir}" >>OpenCL.pc

  if [[ $TARGET == linux* ]]; then
    echo "Libs: -L\${libdir} -lOpenCL" >>OpenCL.pc
    echo "Libs.private: -ldl" >>OpenCL.pc
  elif [[ $TARGET == win* ]]; then
    echo "Libs: -L\${libdir} -l:OpenCL.a" >>OpenCL.pc
    echo "Libs.private: -lole32 -lshlwapi -lcfgmgr32" >>OpenCL.pc
  fi

  mkdir -p "$FFBUILD_PREFIX"/lib/pkgconfig
  mv OpenCL.pc "$FFBUILD_PREFIX"/lib/pkgconfig/OpenCL.pc
}
