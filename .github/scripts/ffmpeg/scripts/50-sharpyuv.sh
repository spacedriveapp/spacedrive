#!/usr/bin/env -S bash -euo pipefail

echo "Download sharpyuv..."
mkdir -p sharpyuv/build

curl -LSs 'https://github.com/mesonbuild/wrapdb/releases/download/libwebp_1.3.2-1/libwebp-1.3.2.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C sharpyuv

curl -LSs 'https://github.com/mesonbuild/wrapdb/releases/download/libwebp_1.3.2-1/libwebp_1.3.2-1_patch.zip' \
  | bsdtar -xf- --strip-component 1 -C sharpyuv

cd sharpyuv/build

echo "Build sharpyuv..."

meson \
  -Dsimd=enabled \
  -Dthreads=enabled \
  -Dlibsharpyuv=enabled \
  -Dcwebp=disabled \
  -Ddwebp=disabled \
  -Dwebpmux=disabled \
  -Dlibwebp=disabled \
  -Dwebpinfo=disabled \
  -Dlibwebpmux=disabled \
  -Dlibwebpdemux=disabled \
  -Dlibwebpdecoder=disabled \
  ..

ninja -j"$(nproc)"

ninja install
