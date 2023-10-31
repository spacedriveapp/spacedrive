#!/usr/bin/env -S bash -euo pipefail

echo "Download theora..."
mkdir -p theora

curl_tar 'https://downloads.xiph.org/releases/theora/libtheora-1.1.1.tar.bz2' theora 1

curl_tar 'https://github.com/mesonbuild/wrapdb/releases/download/theora_1.1.1-4/theora_1.1.1-4_patch.zip' theora 1

sed -i "/subdir('doc')/d" theora/meson.build
sed -i "/subdir('tests')/d" theora/meson.build

# Remove some superfluous files
rm -rf theora/{CHANGES,depcomp,missing,symbian,configure.ac,Makefile.in,config.sub,config.guess,m4,macosx,tests,ltmain.sh,examples,aclocal.m4,configure,doc}

# Backup source
bak_src 'theora'

mkdir -p theora/build
cd theora/build

echo "Build theora..."

meson \
  -Dasm=enabled \
  -Ddoc=disabled \
  -Dspec=disabled \
  -Dexamples=disabled \
  ..

ninja -j"$(nproc)"

ninja install
