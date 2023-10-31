#!/usr/bin/env -S bash -euo pipefail

echo "Download lame..."
mkdir -p lambuild

# TODO: Create patch for sse2neon on lame
# https://github.com/search?q=repo%3Adespoa%2FLAME+sse+language%3AC&type=code
curl_tar 'https://github.com/mesonbuild/wrapdb/releases/download/lame_3.100-9/lame-3.100.tar.gz' lame 1

curl_tar 'https://github.com/mesonbuild/wrapdb/releases/download/lame_3.100-9/lame_3.100-9_patch.zip' lame 1

# Remove some superfluous files
rm -rf lame/{acinclude.m4,config.h.in,testcase.mp3,install-sh,Makefile.MSVC,Makefile.unix,config.rpath,depcomp,Makefile.in,config.sub,configure.in,config.guess,testcase.wav,debian,macosx,Dll,misc,vc_solution,dshow,mac,ltmain.sh,doc,aclocal.m4,frontend,ACM,configure}

# Backup source
bak_src 'lame'

mkdir -p lame/build
cd lame/build

echo "Build lame..."

meson \
  -Dtools=disabled \
  -Ddecoder=false \
  ..

ninja -j"$(nproc)"

ninja install
