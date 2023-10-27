#!/usr/bin/env -S bash -euo pipefail

echo "Download xxhash..."
mkdir -p xxhash/build

curl -LSs 'https://github.com/Cyan4973/xxHash/archive/refs/tags/v0.8.2.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C xxhash

# Patch cmake script to not use CMAKE_HOST_SYSTEM_INFORMATION, as that returns incorrect info when cross compiling
# shellcheck disable=SC2016
sed -i \
  's/CMAKE_HOST_SYSTEM_INFORMATION(RESULT PLATFORM QUERY OS_PLATFORM)/set(PLATFORM "${CMAKE_SYSTEM_PROCESSOR}")/' \
  xxhash/cmake_unofficial/CMakeLists.txt

cd xxhash/build

echo "Build xxhash..."
cmake \
  -DXXHASH_BUILD_XXHSUM=Off \
  ../cmake_unofficial

ninja -j"$(nproc)"

ninja install
