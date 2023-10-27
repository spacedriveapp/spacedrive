#!/usr/bin/env -S bash -euo pipefail

echo "Download opencl..."
mkdir -p opencl/build

curl -LSs 'https://github.com/KhronosGroup/OpenCL-Headers/archive/refs/tags/v2023.04.17.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C "${PREFIX}/include" OpenCL-Headers-2023.04.17/CL

curl -LSs 'https://github.com/KhronosGroup/OpenCL-ICD-Loader/archive/refs/tags/v2023.04.17.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C opencl

cd opencl/build

echo "Build opencl..."
cmake \
  -DOPENCL_ICD_LOADER_HEADERS_DIR="${PREFIX}/include" \
  -DOPENCL_ICD_LOADER_BUILD_SHARED_LIBS=OFF \
  -DOPENCL_ICD_LOADER_PIC=ON \
  -DOPENCL_ICD_LOADER_BUILD_TESTING=OFF \
  -DENABLE_OPENCL_LAYERINFO=Off \
  -DBUILD_TESTING=OFF \
  ..

ninja -j"$(nproc)"

ninja install

case "$TARGET" in
  *linux*)
    LIBS='-lOpenCL'
    LIBS_P='-pthread -ldl'
    ;;
  *windows*)
    LIBS='-lOpenCL'
    LIBS_P='-lole32 -lshlwapi -lcfgmgr32'
    ;;
esac

mkdir -p "${PREFIX}/lib/pkgconfig"
cat <<EOF >"${PREFIX}/lib/pkgconfig/OpenCL.pc"
prefix=$PREFIX
exec_prefix=\${prefix}
libdir=\${exec_prefix}/lib
includedir=\${prefix}/include

Name: OpenCL
Description: OpenCL ICD Loader
Version: 9999
Cflags: -I\${includedir} -DCL_TARGET_OPENCL_VERSION=120
Libs: -L\${libdir} $LIBS
Libs.private: $LIBS_P
EOF

if [ -f "${PREFIX}/lib/OpenCL.a" ] && ! [ -f "${PREFIX}/lib/libOpenCL.a" ]; then
  ln -s OpenCL.a "${PREFIX}/lib/libOpenCL.a"
fi
