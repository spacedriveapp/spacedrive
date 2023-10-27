#!/usr/bin/env -S bash -euo pipefail

echo "Download placebo..."
mkdir -p placebo/build

curl -LSs 'https://github.com/haasn/libplacebo/archive/refs/tags/v5.264.1.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C placebo

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

# Configure required GL versions
sed -i 's/--api=gl:core,gles2,egl/--api=gl:core=4.1,gles2=3.0,egl/' placebo/src/opengl/include/glad/meson.build

# Thrid party deps
curl -LSs 'https://github.com/Dav1dde/glad/archive/refs/tags/v2.0.4.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C placebo/3rdparty/glad
curl -LSs 'https://github.com/pallets/jinja/archive/refs/tags/3.1.2.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C placebo/3rdparty/jinja
curl -LSs 'https://github.com/pallets/markupsafe/archive/refs/tags/2.1.1.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C placebo/3rdparty/markupsafe

cd placebo/build

echo "Build placebo..."

case "$TARGET" in
  *linux*)
    _dx11=disabled
    ;;
  *windows*)
    _dx11=enabled
    ;;
esac

meson \
  -Dlcms=enabled \
  -Dopengl=enabled \
  -Dvulkan=enabled \
  -Dshaderc=enabled \
  -Dunwind=enabled \
  -Dd3d11="${_dx11}" \
  -Dvulkan-registry=/srv/vulkan/registry/vk.xml \
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
