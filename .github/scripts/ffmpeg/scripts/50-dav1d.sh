#!/usr/bin/env -S bash -euo pipefail

echo "Download dav1d..."
mkdir -p dav1d/build

curl -LSs 'https://github.com/videolan/dav1d/archive/refs/tags/1.3.0.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C dav1d

cd dav1d/build

echo "Build dav1d..."
meson \
  -Denable_docs=false \
  -Denable_tools=false \
  -Denable_tests=false \
  -Denable_examples=false \
  ..

ninja -j"$(nproc)"

ninja install
