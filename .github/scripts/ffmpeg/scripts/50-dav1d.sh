#!/usr/bin/env -S bash -euo pipefail

echo "Download dav1d..."
mkdir -p dav1d

curl_tar 'https://github.com/videolan/dav1d/archive/refs/tags/1.3.0.tar.gz' dav1d 1

sed -i "/subdir('doc')/d" dav1d/meson.build
sed -i "/subdir('tools')/d" dav1d/meson.build
sed -i "/subdir('tests')/d" dav1d/meson.build
sed -i "/subdir('examples')/d" dav1d/meson.build

# Remove some superfluous files
rm -rf dav1d/{.github,package,doc,examples,tools,tests}

# Backup source
bak_src 'dav1d'

mkdir -p dav1d/build
cd dav1d/build

echo "Build dav1d..."
meson \
  -Denable_docs=false \
  -Denable_tools=false \
  -Denable_tests=false \
  -Denable_examples=false \
  ..

ninja -j"$(nproc)"

ninja install
