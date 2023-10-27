#!/usr/bin/env -S bash -euo pipefail

echo "Download vorbis..."
mkdir -p vorbis/build

curl -LSs 'https://github.com/xiph/vorbis/releases/download/v1.3.7/libvorbis-1.3.7.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C vorbis

cd vorbis/build

echo "Build vorbis..."
cmake \
  -DBUILD_TESTING=Off \
  -DINSTALL_CMAKE_PACKAGE_MODULE=On \
  ..

ninja -j"$(nproc)"

ninja install
