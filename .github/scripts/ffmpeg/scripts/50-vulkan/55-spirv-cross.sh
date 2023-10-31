#!/usr/bin/env -S bash -euo pipefail

echo "Download spirv..."
mkdir -p spirv

curl_tar 'https://github.com/KhronosGroup/SPIRV-Cross/archive/refs/tags/sdk-1.3.261.1.tar.gz' spirv 1

VERSION="$(
  grep -Po 'set\(spirv-cross-abi-major\s+\K\d+' spirv/CMakeLists.txt
).$(
  grep -Po 'set\(spirv-cross-abi-minor\s+\K\d+' spirv/CMakeLists.txt
).$(
  grep -Po 'set\(spirv-cross-abi-patch\s+\K\d+' spirv/CMakeLists.txt
)"

# Remove some superfluous files
rm -rf spirv/{.github,.reuse,gn,reference,samples,shaders*,tests-other}

# Backup source
bak_src 'spirv'

mkdir -p spirv/build
cd spirv/build

echo "Build spirv..."
cmake \
  -DSPIRV_CROSS_STATIC=On \
  -DSPIRV_CROSS_FORCE_PIC=On \
  -DSPIRV_CROSS_ENABLE_CPP=On \
  -DSPIRV_CROSS_CLI=Off \
  -DSPIRV_CROSS_SHARED=Off \
  -DSPIRV_CROSS_ENABLE_TESTS=Off \
  ..

ninja -j"$(nproc)"

ninja install

cat >"${PREFIX}/lib/pkgconfig/spirv-cross-c-shared.pc" <<EOF
prefix=$PREFIX
exec_prefix=\${prefix}
libdir=\${prefix}/lib
sharedlibdir=\${prefix}/lib
includedir=\${prefix}/include/spirv_cross

Name: spirv-cross-c-shared
Description: C API for SPIRV-Cross
Version: $VERSION

Requires:
Libs: -L\${libdir} -L\${sharedlibdir} -lspirv-cross-c -lspirv-cross-glsl -lspirv-cross-hlsl -lspirv-cross-reflect -lspirv-cross-msl -lspirv-cross-util -lspirv-cross-core -lstdc++
Cflags: -I\${includedir}
EOF
