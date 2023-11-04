#!/usr/bin/env -S bash -euo pipefail

if ! {
  [ "$(uname -m)" = "${TARGET%%-*}" ] && (case "$TARGET" in *linux* | x86_64-windows*) exit 0 ;; *) exit 1 ;; esac)
} then
  export UNSUPPORTED=1
  exit 1
fi

echo "Download nvenv..."
mkdir -p nvenv

curl_tar 'https://github.com/FFmpeg/nv-codec-headers/releases/download/n12.1.14.0/nv-codec-headers-12.1.14.0.tar.gz' nvenv 1

# Backup source
bak_src 'nvenv'

cd nvenv

echo "Copy nvenv headers..."
make PREFIX="$PREFIX" install
