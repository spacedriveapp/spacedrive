#!/usr/bin/env -S bash -euo pipefail

echo "Download zimg..."
mkdir -p zimg

curl_tar 'https://github.com/sekrit-twc/zimg/archive/refs/tags/release-3.0.5.tar.gz' zimg 1

case "$TARGET" in
  *windows*)
    sed -ie 's/#include <Windows.h>/#include <windows.h>/' zimg/src/zimg/common/arm/cpuinfo_arm.cpp
    ;;
esac

sed -i '/^dist_example_DATA/,/src\/testcommon\/win32_bitmap.h/d;' zimg/Makefile.am

# Remove unused components
rm -r zimg/{doc,_msvc,test,src/{testapp,testcommon}}

# Backup source
bak_src 'zimg'

cd zimg

echo "Build zimg..."

./autogen.sh

# shellcheck disable=SC2046
./configure \
  $(
    case "$TARGET" in
      *linux*)
        echo "--host=${TARGET%%-*}-linux-gnu"
        ;;
      *windows*)
        # Always use gnu because mingw64 requires MSVC tools that we do not have
        echo "--host=${TARGET%%-*}-windows-gnu"
        ;;
      x86_64-darwin*)
        echo "--host=${APPLE_TARGET}"
        ;;
      aarch64-darwin*)
        echo "--host=${APPLE_TARGET}"
        ;;
    esac
  ) \
  --prefix="$PREFIX" \
  --with-pic \
  --enable-static \
  --disable-debug \
  --disable-shared \
  --disable-testapp \
  --disable-example \
  --disable-unit-test

make -j"$(nproc)"

make install
