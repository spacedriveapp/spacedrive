#!/usr/bin/env -S bash -euo pipefail

echo "Download placebo..."
mkdir -p placebo/build

curl -LSs 'https://github.com/haasn/libplacebo/archive/refs/tags/v6.338.1.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C placebo

# Fix spirv import
sed -ie 's@spirv_cross_c.h@spirv_cross/spirv_cross_c.h@' placebo/src/d3d11/gpu.h

# Configure requied GL versions
sed -i 's/--api=gl:core,gles2,egl/--api=gl:core=4.1,gles2=3.0,egl/' placebo/src/opengl/include/glad/meson.build

# Thrid party deps
curl -LSs 'https://github.com/Dav1dde/glad/archive/refs/tags/v2.0.4.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C placebo/3rdparty/glad
curl -LSs 'https://github.com/pallets/jinja/archive/refs/tags/3.1.2.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C placebo/3rdparty/jinja
curl -LSs 'https://github.com/pallets/markupsafe/archive/refs/tags/2.1.1.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C placebo/3rdparty/markupsafe
curl -LSs 'https://github.com/fastfloat/fast_float/archive/refs/tags/v5.2.0.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C placebo/3rdparty/fast_float

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
  -Dopengl=enabled \
  -Dgl-proc-addr=disabled \
  -Dvulkan=enabled \
  -Dvk-proc-addr=disabled \
  -Dvulkan-registry=/srv/vulkan/registry/vk.xml \
  -Dshaderc=enabled \
  -Dglslang=disabled \
  -Dd3d11="${_dx11}" \
  -Ddemos=false \
  -Dtests=false \
  -Dbench=false \
  -Dxxhash=enabled \
  -Dunwind=enabled \
  -Dfuzz=false \
  ..

ninja -j"$(nproc)"

ninja install
