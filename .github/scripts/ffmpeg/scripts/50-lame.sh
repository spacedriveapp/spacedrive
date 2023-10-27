#!/usr/bin/env -S bash -euo pipefail

echo "Download lame..."
mkdir -p lame/buuild

# TODO: Create patch for sse2neon on lame
# https://github.com/search?q=repo%3Adespoa%2FLAME+sse+language%3AC&type=code
curl -LSs 'https://github.com/mesonbuild/wrapdb/releases/download/lame_3.100-9/lame_3.100-9_patch.zip' \
  | bsdtar -xf- --strip-component 1 -C lame

curl -LSs 'https://github.com/mesonbuild/wrapdb/releases/download/lame_3.100-9/lame-3.100.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C lame

cd lame/buuild

echo "Build lame..."

meson \
  -Dtools=disabled \
  -Ddecoder=false \
  ..

ninja -j"$(nproc)"

ninja install
