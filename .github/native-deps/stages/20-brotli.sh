#!/usr/bin/env -S bash -euo pipefail

echo "Download brotli..."
mkdir -p brotli

curl_tar 'https://github.com/google/brotli/archive/refs/tags/v1.1.0.tar.gz' brotli 1

# Add meson build support
curl_tar 'https://github.com/mesonbuild/wrapdb/releases/download/google-brotli_1.1.0-1/google-brotli_1.1.0-1_patch.zip' brotli 1

sed -i '/^executable(/,/^)/d;' brotli/meson.build

# Remove unused components
rm -r brotli/{setup.py,CMakeLists.txt,tests,docs,python}

# Backup source
bak_src 'brotli'

mkdir -p brotli/build
cd brotli/build

echo "Build brotli..."
if ! meson ..; then
  cat meson-logs/meson-log.txt
  exit 1
fi

ninja -j"$(nproc)"

ninja install
