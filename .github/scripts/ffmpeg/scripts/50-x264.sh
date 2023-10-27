#!/usr/bin/env -S bash -euo pipefail

echo "Download x264..."
mkdir -p x264

curl -LSs 'https://code.videolan.org/videolan/x264/-/archive/9c3c71688226fbb23f4d36399fab08f018e760b0/x264-9c3c71688226fbb23f4d36399fab08f018e760b0.tar.bz2' \
  | bsdtar -xf- --strip-component 1 -C x264

cd x264

echo "Build x264..."

# shellcheck disable=SC2046
./configure \
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
