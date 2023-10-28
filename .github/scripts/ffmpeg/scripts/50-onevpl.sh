#!/usr/bin/env -S bash -euo pipefail

echo "Download oneVPL..."
mkdir -p oneVPL/build

curl -LSs 'https://github.com/oneapi-src/oneVPL/archive/refs/tags/v2023.3.1.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C oneVPL

cd oneVPL/build

echo "Build oneVPL..."
cmake \
  -DBUILD_DEV=On \
  -DBUILD_TOOLS=Off \
  -DBUILD_TESTS=Off \
  -DBUILD_PREVIEW=Off \
  -DBUILD_EXAMPLES=Off \
  -DBUILD_DISPATCHER=On \
  -DINSTALL_EXAMPLE_CODE=Off \
  -DBUILD_TOOLS_ONEVPL_EXPERIMENTAL=Off \
  -DBUILD_DISPATCHER_ONEVPL_EXPERIMENTAL=Off \
  ..

ninja -j"$(nproc)"

ninja install
