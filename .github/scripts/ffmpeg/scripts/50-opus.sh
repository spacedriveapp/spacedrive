#!/usr/bin/env -S bash -euo pipefail

echo "Download opus..."
mkdir -p opus/build

curl -LSs 'https://github.com/xiph/opus/releases/download/v1.4/opus-1.4.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C opus

# Required patch to fix meson for arm builds
curl -LSs "https://github.com/xiph/opus/commit/20c032d27c59d65b19b8ffbb2608e5282fe817eb.patch" \
| patch -F5 -lp1 -d opus -t

cd opus/build

echo "Build opus..."
meson \
  -Dintrinsics=enabled \
  -Ddocs=disabled \
  -Dtests=disabled \
  -Dextra-programs=disabled \
  ..

ninja -j"$(nproc)"

ninja install
