#!/usr/bin/env -S bash -euo pipefail

echo "Download brotli..."
mkdir -p brotli/build

curl -LSs 'https://github.com/google/brotli/archive/refs/tags/v1.1.0.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C brotli

curl -LSs 'https://github.com/mesonbuild/wrapdb/releases/download/google-brotli_1.1.0-1/google-brotli_1.1.0-1_patch.zip' \
  | bsdtar -xf- --strip-component 1 -C brotli

cd brotli/build

echo "Build brotli..."
meson ..

ninja -j"$(nproc)"

ninja install
