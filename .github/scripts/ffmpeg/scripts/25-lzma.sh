#!/usr/bin/env -S bash -euo pipefail

echo "Download lzma..."
mkdir -p lzma/build

curl -LSs 'https://github.com/tukaani-project/xz/releases/download/v5.4.4/xz-5.4.4.tar.xz' \
  | bsdtar -xf- --strip-component 1 -C lzma

cd lzma/build

echo "Build lzma..."

cmake \
  -DENABLE_SMALL=On \
  -DBUILD_TESTING=Off \
  -DCREATE_XZ_SYMLINKS=Off \
  -DCREATE_LZMA_SYMLINKS=Off \
  -DCMAKE_SKIP_INSTALL_ALL_DEPENDENCY=On \
  ..

ninja -j"$(nproc)" liblzma

case "$TARGET" in
  *windows*)
    touch xz.exe xzdec.exe
    ;;
  *)
    touch xz xzdec
    ;;
esac

ninja install
