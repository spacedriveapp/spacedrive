#!/usr/bin/env -S bash -euo pipefail

echo "Download ogg..."
mkdir -p ogg

curl_tar 'https://github.com/xiph/ogg/releases/download/v1.3.5/libogg-1.3.5.tar.gz' ogg 1

# Remove some superfluous files
rm -rf ogg/{.github,install-sh,depcomp,Makefile.in,config.sub,aclocal.m4,config.guess,ltmain.sh,m4,configure,doc}

# Backup source
bak_src 'ogg'

mkdir -p ogg/build
cd ogg/build

echo "Build ogg..."
cmake \
  -DINSTALL_DOCS=Off \
  -DBUILD_TESTING=Off \
  -DINSTALL_PKG_CONFIG_MODULE=On \
  -DINSTALL_CMAKE_PACKAGE_MODULE=On \
  ..

ninja -j"$(nproc)"

ninja install
