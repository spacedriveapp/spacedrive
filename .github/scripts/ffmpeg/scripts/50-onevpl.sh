#!/usr/bin/env -S bash -euo pipefail

echo "Download oneVPL..."
mkdir -p oneVPL/build

curl -LSs 'https://github.com/oneapi-src/oneVPL/archive/refs/tags/v2023.3.1.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C oneVPL

cd oneVPL/build

echo "Build oneVPL..."
cmake \
  -DBUILD_DEV=ON \
  -DBUILD_TOOLS=OFF \
  -DBUILD_TESTS=OFF \
  -DBUILD_PREVIEW=OFF \
  -DBUILD_EXAMPLES=OFF \
  -DBUILD_DISPATCHER=ON \
  -DINSTALL_EXAMPLE_CODE=OFF \
  -DBUILD_TOOLS_ONEVPL_EXPERIMENTAL=OFF \
  -DBUILD_DISPATCHER_ONEVPL_EXPERIMENTAL=Off \
  ..

ninja -j"$(nproc)"

ninja install
