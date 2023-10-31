#!/usr/bin/env -S bash -euo pipefail

echo "Download vulkan..."
mkdir -p vulkan-headers

curl_tar 'https://github.com/KhronosGroup/Vulkan-Headers/archive/refs/tags/v1.3.269.tar.gz' vulkan-headers 1

VERSION="$(
  sed -nr \
    's/#define\s+VK_HEADER_VERSION_COMPLETE\s+VK_MAKE_API_VERSION\(\s*[0-9]+,\s*([0-9]+),\s*([0-9]+),\s*VK_HEADER_VERSION\)/\1.\2/p' \
    vulkan-headers/include/vulkan/vulkan_core.h
).$(
  sed -nr \
    's/#define\s+VK_HEADER_VERSION\s+([0-9]+)/\1/p' \
    vulkan-headers/include/vulkan/vulkan_core.h
)"

# Remove some superfluous files
rm -rf vulkan-headers/{.reuse,.github,tests}

# Backup source
bak_src 'vulkan-headers'

mkdir -p vulkan-headers/build
cd vulkan-headers/build

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
