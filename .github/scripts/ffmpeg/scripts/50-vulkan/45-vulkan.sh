#!/usr/bin/env -S bash -euo pipefail

echo "Download vulkan..."
mkdir -p vulkan/build

curl -LSs 'https://github.com/KhronosGroup/Vulkan-Headers/archive/refs/tags/v1.3.269.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C vulkan

VERSION="$(
  sed -nr \
    's/#define\s+VK_HEADER_VERSION_COMPLETE\s+VK_MAKE_API_VERSION\(\s*[0-9]+,\s*([0-9]+),\s*([0-9]+),\s*VK_HEADER_VERSION\)/\1.\2/p' \
    vulkan/include/vulkan/vulkan_core.h
).$(
  sed -nr \
    's/#define\s+VK_HEADER_VERSION\s+([0-9]+)/\1/p' \
    vulkan/include/vulkan/vulkan_core.h
)"

cd vulkan/build

echo "Build vulkan..."
cmake \
  -DBUILD_TESTS=Off \
  ..

ninja -j"$(nproc)"

ninja install

cat >"$PREFIX"/lib/pkgconfig/vulkan.pc <<EOF
prefix=$PREFIX
includedir=\${prefix}/include

Name: vulkan
Version: $VERSION
Description: Vulkan (Headers Only)
Cflags: -I\${includedir}
EOF
