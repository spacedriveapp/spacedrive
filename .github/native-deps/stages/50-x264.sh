#!/usr/bin/env -S bash -euo pipefail

echo "Download x264..."
mkdir -p x264

# Using master due to aarch64 improvements
curl_tar 'https://code.videolan.org/videolan/x264/-/archive/d46938de/x264.tar.bz2' x264 1

case "$TARGET" in
  *darwin*)
    sed -i "/^if cc_check '' '' '' '__attribute__((force_align_arg_pointer))' ; then/,/^fi/d;" x264/configure
    ;;
esac

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
    if [ "${LTO:-1}" -eq 1 ]; then
      echo '--enable-lto'
    fi

    case "$TARGET" in
      *linux*)
        echo "--host=${TARGET%%-*}-linux-gnu"
        echo '--disable-win32thread'
        ;;
      *windows*)
        echo "--host=${TARGET%%-*}-windows-mingw64"
        ;;
      x86_64-darwin*)
        echo '--host=x86_64-apple-darwin19'
        echo '--disable-win32thread'
        # FIX-ME: x264 x86 asm causes ld64.lld (macOS) to segfault
        echo '--disable-asm'
        ;;
      aarch64-darwin*)
        echo '--host=aarch64-apple-darwin20'
        echo '--disable-win32thread'
        # FIX-ME: x264 aarch64 asm causes ld64.lld (macOS) to segfault
        echo '--disable-asm'
        ;;
    esac
  ) \
  --enable-pic \
  --enable-static \
  --bit-depth=all \
  --chroma-format=all \
  --disable-cli

make -j"$(nproc)"

make install
