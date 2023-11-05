#!/usr/bin/env bash

set -euo pipefail

case "$TARGET" in
  *darwin*) ;;
  *)
    exit 0
    ;;
esac

export CC="clang-16"
export CXX="clang++-16"
export CFLAGS="-I${CCTOOLS}/include"
export LDFLAGS="-L${CCTOOLS}/lib"
export APPLE_TARGET='__BYPASS__'

cd /srv

# LLVM install path
export INSTALLPREFIX="$CCTOOLS"

echo "Download libtapi ..."

mkdir -p "libtapi"

curl_tar 'https://github.com/tpoechtrager/apple-libtapi/archive/43a0c04bcd1f805f55a128744f24e4eed051e681.tar.gz' \
  'libtapi' 1

cd libtapi

./build.sh
./install.sh
