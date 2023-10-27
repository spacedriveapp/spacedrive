#!/usr/bin/env -S bash -euo pipefail

echo "Download bzip2..."
mkdir -p bzip2/build

curl -LSs 'https://gitlab.com/bzip2/bzip2/-/archive/66c46b8c9436613fd81bc5d03f63a61933a4dcc3/bzip2-66c46b8c9436613fd81bc5d03f63a61933a4dcc3.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C bzip2

cd bzip2/build

echo "Build bzip2..."
cmake \
  -DENABLE_APP=Off \
  -DENABLE_TESTS=Off \
  -DENABLE_DOCS=Off \
  -DENABLE_EXAMPLES=Off \
  -DENABLE_STATIC_LIB=On \
  -DENABLE_SHARED_LIB=Off \
  -DUSE_OLD_SONAME=On \
  -DENABLE_STATIC_LIB_IS_PIC=On \
  ..

ninja -j"$(nproc)"

ninja install

ln -s libbz2_static.a "${PREFIX}/lib/libbz2.a"
