#!/usr/bin/env -S bash -euo pipefail

echo "Download zimg..."
mkdir -p zimg

curl -LSs 'https://github.com/sekrit-twc/zimg/archive/refs/tags/release-3.0.5.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C zimg

sed -ie 's/#include <Windows.h>/#include <windows.h>/' zimg/src/zimg/common/arm/cpuinfo_arm.cpp

cd zimg

echo "Build zimg..."

export CFLAGS="${CFLAGS:-} -Os"
export CXXFLAGS="${CFLAGS}"

./autogen.sh

./configure \
  --host="$TARGET" \
  --prefix="$PREFIX" \
  --with-pic \
  --enable-static \
  --disable-shared

make -j"$(nproc)"

make install
