#!/usr/bin/env -S bash -euo pipefail

echo "Download zlib..."
mkdir -p zlib/build

curl -LSs 'https://github.com/zlib-ng/zlib-ng/archive/refs/tags/2.1.4.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C zlib

# Patch cmake to disable armv6 assembly, it thinks we support it, but we don't
sed -i 's/HAVE_ARMV6_INLINE_ASM OR HAVE_ARMV6_INTRIN/HAVE_ARMV6_INLINE_ASM AND HAVE_ARMV6_INTRIN/' zlib/CMakeLists.txt

cd zlib/build

echo "Build zlib..."

cmake \
  -DWITH_OPTIM=On \
  -DZLIB_COMPAT=On \
  -WITH_NATIVE_INSTRUCTIONS=On \
  -DWITH_GTEST=Off \
  -DWITH_NATIVE_INSTRUCTIONS=Off \
  -DWITH_FUZZERS=Off \
  -DWITH_REDUCED_MEM=Off \
  -DWITH_BENCHMARK_APPS=Off \
  -DWITH_BENCHMARKS=Off \
  -DZLIB_ENABLE_TESTS=Off \
  -DZLIBNG_ENABLE_TESTS=Off \
  -DINSTALL_UTILS=Off \
  ..

ninja -j"$(nproc)"

ninja install
