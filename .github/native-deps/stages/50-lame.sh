#!/usr/bin/env -S bash -euo pipefail

echo "Download lame..."
mkdir -p lambuild

# TODO: Create patch for sse2neon on lame
# Original link: https://sourceforge.net/projects/lame/files/lame/3.100/lame-3.100.tar.gz/download
# But sourcefourge is very bad to download from, so we use the debian source instead
curl_tar 'https://deb.debian.org/debian/pool/main/l/lame/lame_3.100.orig.tar.gz' lame 1

# Add meson build support
curl_tar 'https://github.com/mesonbuild/wrapdb/releases/download/lame_3.100-9/lame_3.100-9_patch.zip' lame 1

# Fix warning on 64 bit machines. explicitly set variables as unsigned ints.
curl -LSs 'https://sources.debian.org/data/main/l/lame/3.100-6/debian/patches/07-field-width-fix.patch' \
  | patch -F5 -lp1 -d lame -t

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

cat << EOF >"${PREFIX}/lib/pkgconfig/lame.pc"
prefix=$PREFIX
exec_prefix=\${prefix}
libdir=\${exec_prefix}/lib
includedir=\${prefix}/include

Name: lame
Description: high quality MPEG Audio Layer III (MP3) encoder library
Version: 3.100
Libs: -L\${libdir} -lmp3lame
Cflags: -I\${includedir}/lame
EOF
