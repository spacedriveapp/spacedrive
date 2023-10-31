#!/usr/bin/env -S bash -euo pipefail

echo "Download vpx..."
mkdir -p vpx

# v1.13.1
curl_tar 'https://gitlab.freedesktop.org/gstreamer/meson-ports/libvpx/-/archive/b2bd418b/libvpx.tar.gz' vpx 1

# Remove some superfluous files
rm -rf vpx/{third_party/googletest,build_debug,test,tools,examples,examples.mk,configure,*.dox,.gitlab*}

# Backup source
bak_src 'vpx'

mkdir -p vpx/build
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
