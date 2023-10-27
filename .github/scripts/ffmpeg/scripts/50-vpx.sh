#!/usr/bin/env -S bash -euo pipefail

echo "Download vpx..."
mkdir -p vpx/build

# v1.13.1
curl -LSs 'https://gitlab.freedesktop.org/gstreamer/meson-ports/libvpx/-/archive/b2bd418b6f3bc28eedd8f94681cac5c1e4e5eb00/libvpx-b2bd418b6f3bc28eedd8f94681cac5c1e4e5eb00.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C vpx

cd vpx/build

echo "Build vpx..."

meson \
  -Dvp8=enabled \
  -Dvp9=enabled \
  -Dlibs=enabled \
  -Dvp8_decoder=enabled \
  -Dvp9_decoder=enabled \
  -Dvp8_encoder=enabled \
  -Dvp9_encoder=enabled \
  -Dmultithread=enabled \
  -Dinstall_libs=enabled \
  -Dvp9_highbitdepth=enabled \
  -Dbetter_hw_compatibility=enabled \
  -Ddocs=disabled \
  -Dtools=disabled \
  -Dgprof=disabled \
  -Dexamples=disabled \
  -Dinstall_docs=disabled \
  -Dinstall_bins=disabled \
  -Dunit_tests=disabled \
  -Dinternal_stats=disabled \
  -Ddecode_perf_tests=disabled \
  -Dencode_perf_tests=disabled \
  ..

ninja -j"$(nproc)"

ninja install
