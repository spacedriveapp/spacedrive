#!/usr/bin/env -S bash -euo pipefail

echo "Download x264..."
mkdir -p x264

# Using master due to aarch64 improvements
curl_tar 'https://code.videolan.org/videolan/x264/-/archive/d46938de/x264.tar.bz2' x264 1

# Remove some superfluous files
rm -rf x264/doc

# Backup source
bak_src 'x264'

cd x264

echo "Build x264..."

# x264 is only compatible with windres, so use compat script
# shellcheck disable=SC2046
env RC="$WINDRES" ./configure \
  --prefix="$PREFIX" \
  $(
    case "$TARGET" in
      *linux*)
        echo "--host=${TARGET%%-*}-linux-gnu"
        echo '--disable-win32thread'
        ;;
      *windows*)
        echo "--host=${TARGET%%-*}-windows-mingw64"
        ;;
    esac
  ) \
  --enable-lto \
  --enable-pic \
  --enable-static \
  --bit-depth=all \
  --chroma-format=all \
  --disable-cli

make -j"$(nproc)"

make install
