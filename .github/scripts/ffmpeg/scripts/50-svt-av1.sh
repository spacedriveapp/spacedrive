#!/usr/bin/env -S bash -euo pipefail

echo "Download svt-av1..."
mkdir -p svt-av1/build

curl -LSs 'https://gitlab.com/AOMediaCodec/SVT-AV1/-/archive/v1.7.0/SVT-AV1-v1.7.0.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C svt-av1

case "$TARGET" in
  x86_64*)
    ENABLE_NASM=On
    ;;
  aarch64*)
    ENABLE_NASM=Off
    # Patch to enable SSE in aarch64
    curl -LSs 'https://raw.githubusercontent.com/HandBrake/HandBrake/621a6ff/contrib/svt-av1/A01-adds-neon-sse2neon-implementations-of-SVT-AV1_v15.patch' \
      | patch -F5 -lp1 -d svt-av1 -t
    ;;
esac

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
