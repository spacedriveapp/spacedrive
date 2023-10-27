#!/usr/bin/env -S bash -euo pipefail

echo "Download ogg..."
mkdir -p ogg/build

curl -LSs 'https://github.com/xiph/ogg/releases/download/v1.3.5/libogg-1.3.5.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C ogg

cd ogg/build

echo "Build ogg..."
cmake \
  -DINSTALL_DOCS=Off \
  -DBUILD_TESTING=Off \
  -DINSTALL_PKG_CONFIG_MODULE=On \
  -DINSTALL_CMAKE_PACKAGE_MODULE=On \
  ..

ninja -j"$(nproc)"

ninja install
