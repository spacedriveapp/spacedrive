#!/usr/bin/env -S bash -euo pipefail

echo "Download brotli..."
mkdir -p brotli

curl_tar 'https://github.com/google/brotli/archive/refs/tags/v1.1.0.tar.gz' brotli 1

curl_tar 'https://github.com/mesonbuild/wrapdb/releases/download/google-brotli_1.1.0-1/google-brotli_1.1.0-1_patch.zip' brotli 1

# Remove unused components
rm -r brotli/{setup.py,CMakeLists.txt,tests,docs,python}

# Backup source
bak_src 'brotli'

mkdir -p brotli/build
cd brotli/build

echo "Build brotli..."
meson ..

ninja -j"$(nproc)"

ninja install
