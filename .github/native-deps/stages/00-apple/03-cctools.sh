#!/usr/bin/env bash

set -euo pipefail

case "$TARGET" in
  x84_64-darwin*)
    _target='x86_64-apple-darwin19'
    ;;
  aarch64-darwin*)
    _target='arm64-apple-darwin20'
    ;;
  *)
    exit 0
    ;;
esac

echo "APPLE_TARGET=$_target" >>"/root/.cache/environment"

apt-get install uuid-dev libedit-dev

export CC="clang-16"
export CXX="clang++-16"
export CFLAGS="-I${CCTOOLS}/include"
export LDFLAGS="-L${CCTOOLS}/lib"
export APPLE_TARGET='__BYPASS__'

cd /srv

echo "Download cctools ..."

mkdir -p "cctools"

curl_tar 'https://github.com/tpoechtrager/cctools-port/archive/437ced391dbf14dce86f977ca050a750d5682f39.tar.gz' \
  'cctools' 1

sed -i "/^if readelf -p .comment \$LIBDIR\/libLTO.so | grep clang &>\/dev\/null; then/,/^fi/d;" cctools/tools/fix_liblto.sh
sed -ie 's/wget/curl -LSsOJ/' cctools/tools/fix_liblto.sh

env LLVM_CONFIG=llvm-config-16 cctools/tools/fix_liblto.sh

cd cctools/cctools

./configure \
  --prefix="$CCTOOLS" \
  --target="$_target" \
  --with-libxar="$CCTOOLS" \
  --with-libtapi="$CCTOOLS" \
  --with-llvm-config=llvm-config-16 \
  --enable-xar-support \
  --enable-lto-support

make -j"$(nproc)"

make install
