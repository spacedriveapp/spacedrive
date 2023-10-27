#!/usr/bin/env -S bash -euo pipefail

case "$TARGET" in
  aarch64-macos* | aarch64-windows*)
    export UNSUPPORTED=1
    exit 1
    ;;
esac

echo "Download nvenv..."
mkdir -p nvenv

curl -LSs 'https://github.com/FFmpeg/nv-codec-headers/releases/download/n12.1.14.0/nv-codec-headers-12.1.14.0.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C nvenv

cd nvenv

echo "Copy nvenv headers..."
make PREFIX="$PREFIX" install
