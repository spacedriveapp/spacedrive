#!/usr/bin/env -S bash -euo pipefail

echo "Download opencl..."

mkdir -p opencl

curl_tar 'https://github.com/KhronosGroup/OpenCL-ICD-Loader/archive/refs/tags/v2023.04.17.tar.gz' opencl 1

# Remove some superfluous files
rm -rf opencl/{.github,test}

# Backup source
bak_src 'opencl'

mkdir -p opencl/build
cd opencl/build

echo "Build opencl..."
cmake \
  -DOPENCL_ICD_LOADER_PIC=On \
  -DOPENCL_ICD_LOADER_HEADERS_DIR="${PREFIX}/include" \
  -DBUILD_TESTING=Off \
  -DENABLE_OPENCL_LAYERINFO=Off \
  -DOPENCL_ICD_LOADER_BUILD_TESTING=Off \
  -DOPENCL_ICD_LOADER_BUILD_SHARED_LIBS=Off \
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
