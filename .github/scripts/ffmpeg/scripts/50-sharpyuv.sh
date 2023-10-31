#!/usr/bin/env -S bash -euo pipefail

echo "Download sharpyuv..."
mkdir -p sharpyuv

curl_tar 'https://github.com/webmproject/libwebp/archive/refs/tags/v1.3.2.tar.gz' sharpyuv 1

curl_tar 'https://github.com/mesonbuild/wrapdb/releases/download/libwebp_1.3.2-1/libwebp_1.3.2-1_patch.zip' sharpyuv 1

# Fix install location for sharpyuv headers
sed -i "s|subdir: 'webp/sharpyuv'|subdir: 'sharpyuv'|" sharpyuv/sharpyuv/meson.build

sed -i "/subdir('examples')/d" sharpyuv/meson.build

# Remove some superfluous files
rm -rf sharpyuv/{.github,.cmake-format.py,PRESUBMIT.py,build.gradle,xcframeworkbuild.sh,.pylintrc,m4,Makefile.vc,makefile.unix,cmake,CMakeLists.txt,configure.ac,infra,extras,man,gradle,doc,swig,examples,tests,webp_js}

# Backup source
bak_src 'sharpyuv'

mkdir -p sharpyuv/build
cd sharpyuv/build

echo "Build sharpyuv..."

meson \
  -Dsimd=enabled \
  -Dthreads=enabled \
  -Dlibsharpyuv=enabled \
  -Dcwebp=disabled \
  -Ddwebp=disabled \
  -Dwebpmux=disabled \
  -Dlibwebp=disabled \
  -Dwebpinfo=disabled \
  -Dlibwebpmux=disabled \
  -Dlibwebpdemux=disabled \
  -Dlibwebpdecoder=disabled \
  ..

ninja -j"$(nproc)"

ninja install
