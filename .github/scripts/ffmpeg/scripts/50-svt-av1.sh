#!/usr/bin/env -S bash -euo pipefail

echo "Download svt-av1..."
mkdir -p svt-av1

curl_tar 'https://gitlab.com/AOMediaCodec/SVT-AV1/-/archive/v1.7.0/SVT-AV1-v1.7.0.tar.gz' svt-av1 1

case "$TARGET" in
  x86_64*)
    ENABLE_NASM=On
    ;;
  aarch64*)
    ENABLE_NASM=Off
    # Patch to enable SSE in aarch64
    curl -LSs 'https://gitlab.com/AOMediaCodec/SVT-AV1/-/merge_requests/2135.patch' | patch -F5 -lp1 -d svt-av1 -t
    ;;
esac

# Remove some superfluous files
rm -rf svt-av1/{Docs,Config,test,ffmpeg_plugin,gstreamer-plugin,.gitlab*}

# Backup source
bak_src 'svt-av1'

mkdir -p svt-av1/build
cd svt-av1/build

echo "Build svt-av1..."
cmake \
  -DBUILD_ENC=On \
  -DSVT_AV1_LTO=On \
  -DENABLE_NASM="${ENABLE_NASM}" \
  -DREPRODUCIBLE_BUILDS=On \
  -DCOVERAGE=Off \
  -DBUILD_DEC=Off \
  -DBUILD_APPS=Off \
  -DBUILD_TESTING=Off \
  ..

ninja -j"$(nproc)"

ninja install
