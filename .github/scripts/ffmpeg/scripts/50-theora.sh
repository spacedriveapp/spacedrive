#!/usr/bin/env -S bash -euo pipefail

echo "Download theora..."
mkdir -p theora/build

curl -LSs 'http://downloads.xiph.org/releases/theora/libtheora-1.1.1.tar.bz2' \
  | bsdtar -xf- --strip-component 1 -C theora

curl -LSs 'https://github.com/mesonbuild/wrapdb/releases/download/theora_1.1.1-4/theora_1.1.1-4_patch.zip' \
  | bsdtar -xf- --strip-component 1 -C theora

cd theora/build

echo "Build theora..."

meson \
  -Dasm=enabled \
  -Ddoc=disabled \
  -Dspec=disabled \
  -Dexamples=disabled \
  ..

ninja -j"$(nproc)"

ninja install
