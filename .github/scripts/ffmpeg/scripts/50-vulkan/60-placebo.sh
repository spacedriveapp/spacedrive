#!/usr/bin/env -S bash -euo pipefail

echo "Download placebo..."
mkdir -p placebo

curl_tar 'https://github.com/haasn/libplacebo/archive/refs/tags/v5.264.1.tar.gz' placebo 1

# Some required patches for fixing log for windows
for patch in \
  https://github.com/haasn/libplacebo/commit/c02a8a2f49b462c4715add17855cc4575d1d2ac7.patch \
  https://github.com/haasn/libplacebo/commit/7f9eb409dc679ee5bf096dbdfd84a5ccfee38b7a.patch \
  https://github.com/haasn/libplacebo/commit/cd7a371a0dcbb0d0cc8795d5c4788d1873332523.patch \
  https://github.com/haasn/libplacebo/commit/dc6e5a5e3deaa5355a91338c9497371f318da308.patch \
  https://github.com/haasn/libplacebo/commit/97d008b6d39a05b619e5b61fac05f4f2ef122ede.patch; do
  curl -LSs "$patch" | patch -F5 -lp1 -d placebo -t
done

# Fix spirv import
sed -ie 's@spirv_cross_c.h@spirv_cross/spirv_cross_c.h@' placebo/src/d3d11/gpu.h

# Thrid party deps
curl_tar 'https://github.com/pallets/jinja/archive/refs/tags/3.1.2.tar.gz' placebo/3rdparty/jinja 1
curl_tar 'https://github.com/pallets/markupsafe/archive/refs/tags/2.1.3.tar.gz' placebo/3rdparty/markupsafe 1

# Remove some superfluous files
rm -rf placebo/{.github,docs,demos}
rm -rf placebo/3rdparty/jinja/{.github,artwork,docs,examples,requirements,scripts,tests}
rm -rf placebo/3rdparty/markupsafe/{.github,bench,docs,requirements,tests}

# Backup source
bak_src 'placebo'

mkdir -p placebo/build
cd placebo/build

echo "Build placebo..."

# Only vulkan is supported by FFmpeg when using libplacebo
meson \
  -Dlcms=enabled \
  -Dopengl=disabled \
  -Dvulkan=enabled \
  -Dshaderc=enabled \
  -Dunwind=enabled \
  -Dd3d11=disabled \
  -Dvulkan-registry=/srv/vulkan-headers/registry/vk.xml \
  -Dglslang=disabled \
  -Dgl-proc-addr=disabled \
  -Dvk-proc-addr=disabled \
  -Dfuzz=false \
  -Ddemos=false \
  -Dtests=false \
  -Dbench=false \
  ..

ninja -j"$(nproc)"

ninja install
