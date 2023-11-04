#!/usr/bin/env -S bash -euo pipefail

echo "Download de265..."
mkdir -p de265

curl_tar 'https://github.com/strukturag/libde265/archive/refs/tags/v1.0.12.tar.gz' de265 1

case "$TARGET" in
  aarch64*)
    # Patch to enable SSE codepath on aarch64
    patch -F5 -lp1 -d de265 -t <"$PREFIX"/patches/sse2neon.patch
    ;;
esac

# Remove unused components
rm -r de265/{.github,dec265,enc265,sherlock265}

# Backup source
bak_src 'de265'

mkdir -p de265/build
cd de265/build

echo "Build de265..."
cmake \
  -DENABLE_SDL=Off \
  -DDISABLE_SSE=Off \
  -DENABLE_DECODER=Off \
  -DENABLE_ENCODER=Off \
  ..

ninja -j"$(nproc)"

ninja install
