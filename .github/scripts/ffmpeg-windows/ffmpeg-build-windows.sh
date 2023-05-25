#!/usr/bin/env bash

set -e          # exit immediate if an error occurs in a pipeline
set -E          # make commands inherit ERR trap
set -u          # don't allow not set variables to be utilized
set -o pipefail # trace ERR through pipes
set -o errtrace # trace ERR through 'time command' and other functions
set -x

#---------------------------------- Constants ---------------------------------/
host_target='x86_64-w64-mingw32'
mingw_bin_path="/srv/mingw-w64-x86_64/bin"

script_path="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd -P)"
cross_prefix="${mingw_bin_path}/x86_64-w64-mingw32-"
mingw_w64_x86_64_prefix="/srv/mingw-w64-x86_64/${host_target}"

export PATH="${mingw_bin_path}:${PATH}"

export AR="${host_target}-ar"
export LD="${host_target}-ld"
export CC="${host_target}-gcc"
export CXX="${host_target}-g++"
export STRIP="${host_target}-strip"
export RANLIB="${host_target}-ranlib"
# high compatible by default, see #219, some other good options are listed below, or you could use -march=native to target your local box:
export CFLAGS='-mtune=generic -O3'
export PREFIX="$mingw_w64_x86_64_prefix"
# Needed for mingw-w64 7 as FORTIFY_SOURCE is now partially implemented, but not actually working
export CPPFLAGS='-U_FORTIFY_SOURCE -D_FORTIFY_SOURCE=0'
export PKG_CONFIG_PATH="${mingw_w64_x86_64_prefix}/lib/pkgconfig"
# disable pkg-config from finding [and using] normal linux system installed libs [yikes]
export PKG_CONFIG_LIBDIR=

#---------------------------------- Functions ---------------------------------/
cmake() {
  local dir="$1"
  shift
  command cmake "$dir" \
    -DCMAKE_RANLIB="${host_target}-ranlib" \
    -DCMAKE_C_COMPILER="${host_target}-gcc" \
    -DBUILD_SHARED_LIBS=0 \
    -DCMAKE_SYSTEM_NAME=Windows \
    -DCMAKE_RC_COMPILER="${host_target}-windres" \
    -DCMAKE_CXX_COMPILER="${host_target}-g++" \
    -DCMAKE_INSTALL_PREFIX="$mingw_w64_x86_64_prefix" \
    -DCMAKE_FIND_ROOT_PATH="$mingw_w64_x86_64_prefix" \
    -DENABLE_STATIC_RUNTIME=1 \
    -DCMAKE_FIND_ROOT_PATH_MODE_LIBRARY=ONLY \
    -DCMAKE_FIND_ROOT_PATH_MODE_INCLUDE=ONLY \
    -DCMAKE_FIND_ROOT_PATH_MODE_PROGRAM=NEVER \
    "$@" \
    -G"Unix Makefiles"
}

configure() {
  if [ -f bootstrap ]; then
    ./bootstrap # some need this to create ./configure :|
  fi

  if ! [ -f ./configure ]; then
    autoreconf -fiv # a handful of them require this to create ./configure :|
  fi

  chmod +x ./configure
  ./configure \
    --host="$host_target" --prefix="$mingw_w64_x86_64_prefix" --disable-shared --enable-static "$@"
}

make_install() {
  make "$@" -j"$(nproc)"
  make install "$@" -j"$(nproc)"
}

gen_ld_script() {
  if [ $# -lt 3 ]; then
    echo "gen_ld_script: Invalid arguments" >&2
    return 1
  fi

  local lib="${mingw_w64_x86_64_prefix}/lib/${1}"
  local lib_s="$2"

  if ! [ -f "${mingw_w64_x86_64_prefix}/lib/lib${lib_s}.a" ]; then
    echo "Generating linker script ${lib}: $2 $3"
    mv -f "$lib" "${mingw_w64_x86_64_prefix}/lib/lib${lib_s}.a"
    echo "GROUP ( -l${lib_s} $3 )" >"$lib"
  fi
}

download_and_unpack_file() {
  if [ $# -lt 2 ]; then
    echo "download_and_unpack_file: Invalid arguments" >&2
    return 1
  fi

  mkdir "$1"
  cd "$1"

  curl -sSfL "$2" \
    | bsdtar -xf- --strip-components=1
}

#------------------------------------ Main ------------------------------------/
mkdir -p "$mingw_w64_x86_64_prefix"

CDPATH='' cd -- /srv

(# build mingw-std-threads
  download_and_unpack_file mingw-std-threads https://github.com/meganz/mingw-std-threads/archive/6c2061b.zip
  cp -av -- *.h "${mingw_w64_x86_64_prefix}/include"
)

(# Build dlfcn
  download_and_unpack_file dlfcn-win32 https://github.com/dlfcn-win32/dlfcn-win32/archive/6444294.zip

  # Change CFLAGS.
  sed -i "s/-O3/-O2/" Makefile

  chmod +x ./configure
  ./configure --prefix="$mingw_w64_x86_64_prefix" --cross-prefix="${host_target}-"

  make_install

  # dlfcn-win32's 'README.md': "If you are linking to the static 'dl.lib' or 'libdl.a',
  # then you would need to explicitly add 'psapi.lib' or '-lpsapi' to your linking command,
  # depending on if MinGW is used."
  gen_ld_script libdl.a dl_s -lpsapi
)

(# build zlib
  download_and_unpack_file zlib https://github.com/madler/zlib/releases/download/v1.2.13/zlib-1.2.13.tar.xz

  export CHOST="$host_target"
  export ARFLAGS=rcs

  chmod +x ./configure
  ./configure --prefix="$mingw_w64_x86_64_prefix" --static

  make_install ARFLAGS="$ARFLAGS"
)

(# build bzip2
  download_and_unpack_file bzip2 https://sourceware.org/pub/bzip2/bzip2-1.0.8.tar.gz

  patch -p0 <"${script_path}/bzip2-1.0.8_brokenstuff.diff"

  make CC="$CC" AR="$AR" PREFIX="$PREFIX" RANLIB="$RANLIB" LD="$LD" STRIP="$STRIP" CXX="$CXX" libbz2.a -j"$(nproc)"

  install -m644 bzlib.h "${mingw_w64_x86_64_prefix}/include/bzlib.h"
  install -m644 libbz2.a "${mingw_w64_x86_64_prefix}/lib/libbz2.a"
)

(# build liblzma, depends: dlfcn
  download_and_unpack_file liblzma https://sourceforge.net/projects/lzmautils/files/xz-5.2.5.tar.xz

  configure --disable-doc \
    --disable-lzmadec \
    --disable-lzmainfo \
    --disable-scripts \
    --disable-xz \
    --disable-xzdec

  make_install
)

(# build iconv, depends: dlfcn
  download_and_unpack_file iconv https://ftp.gnu.org/pub/gnu/libiconv/libiconv-1.17.tar.gz

  configure --disable-nls

  make install-lib -j"$(nproc)"
)

(# build libzimg, depends: dlfcn
  download_and_unpack_file libzimg https://github.com/sekrit-twc/zimg/archive/refs/tags/release-3.0.4.tar.gz

  configure

  make_install
)

(# build libopenjpeg
  download_and_unpack_file libopenjpeg https://github.com/uclouvain/openjpeg/archive/refs/tags/v2.5.0.tar.gz

  cmake . -DBUILD_CODEC=0

  make_install
)

(# build libpng, depends: zlib >= 1.0.4, dlfcn
  download_and_unpack_file libpng https://github.com/glennrp/libpng/archive/refs/tags/v1.6.39.tar.gz

  configure

  make_install
)

(# build libwebp, depends: dlfcn
  # Version 1.3.0
  download_and_unpack_file libwebp https://chromium.googlesource.com/webm/libwebp.git/+archive/b557776962a3dcc985d83bd4ed94e1e2e50d0fa2.tar.gz

  export LIBPNG_CONFIG="${mingw_w64_x86_64_prefix}/bin/libpng-config --static" # LibPNG somehow doesn't get autodetected.

  configure --disable-wic

  make_install
)

(# build freetype, depends: bzip2
  download_and_unpack_file freetype https://sourceforge.net/projects/freetype/files/freetype2/2.13.0/freetype-2.13.0.tar.xz

  # src/tools/apinames.c gets crosscompiled and makes the compilation fail
  patch -p0 <"${script_path}/freetype2-crosscompiled-apinames.diff"

  # harfbuzz autodetect :|
  configure --with-bzip2 --without-harfbuzz

  make_install
)

(# build harfbuzz, depends: zlib, freetype, and libpng.
  download_and_unpack_file harfbuzz https://github.com/harfbuzz/harfbuzz/releases/download/7.3.0/harfbuzz-7.3.0.tar.xz

  export LDFLAGS=-lpthread
  # no fontconfig, don't want another circular what? icu is #372
  configure --with-freetype=yes --with-fontconfig=no --with-icu=no

  make_install

  # Rebuild freetype with harfbuzz
  unset LDFLAGS
  (
    cd /srv/freetype
    configure --with-bzip2
    make -j"$(nproc)"
    make install -j"$(nproc)"
  )

  # for some reason it lists harfbuzz as Requires.private only??
  sed -i 's/-lfreetype.*/-lfreetype -lharfbuzz -lpthread/' "$PKG_CONFIG_PATH/freetype2.pc"
  # does anything even use this?
  sed -i 's/-lharfbuzz.*/-lharfbuzz -lfreetype/' "$PKG_CONFIG_PATH/harfbuzz.pc"
  # XXX what the..needed?
  sed -i 's/libfreetype.la -lbz2/libfreetype.la -lharfbuzz -lbz2/' "${mingw_w64_x86_64_prefix}/lib/libfreetype.la"
  sed -i 's/libfreetype.la -lbz2/libfreetype.la -lharfbuzz -lbz2/' "${mingw_w64_x86_64_prefix}/lib/libharfbuzz.la"
)

(# build libxml2, depends: zlib, liblzma, iconv and dlfcn
  download_and_unpack_file libxml2 http://xmlsoft.org/sources/libxml2-2.9.12.tar.gz

  configure --with-ftp=no --with-http=no --with-python=no

  make_install
)

(# build fontconfig, depends freetype, libxml >= 2.6, iconv and dlfcn.
  download_and_unpack_file fontconfig https://www.freedesktop.org/software/fontconfig/release/fontconfig-2.14.2.tar.xz

  configure "--enable-iconv --enable-libxml2 --disable-docs --with-libiconv" # Use Libxml2 instead of Expat.

  make_install
)

(# build libogg, depends: dlfcn
  download_and_unpack_file libogg https://github.com/xiph/ogg/releases/download/v1.3.5/libogg-1.3.5.tar.xz

  configure

  make_install
)

(# build libvorbis, depends: libogg >= 1.0, dlfcn
  download_and_unpack_file libvorbis https://github.com/xiph/vorbis/releases/download/v1.3.7/libvorbis-1.3.7.tar.xz

  configure --disable-docs --disable-examples --disable-oggtest

  make_install
)

(# build libopus, depends: dlfcn
  download_and_unpack_file opus https://github.com/xiph/opus/releases/download/v1.4/opus-1.4.tar.gz

  configure --disable-doc --disable-extra-programs --disable-stack-protector

  make_install
)

(# build libtheora, depends: libogg >= 1.1, libvorbis >= 1.0.1, libpng, dlfcn.
  download_and_unpack_file libtheora http://downloads.xiph.org/releases/theora/libtheora-1.1.1.tar.bz2

  patch -p0 <"${script_path}/libtheora-1.1.1-libpng16.patch"

  cd theora_git

  # disable asm: avoid [theora @ 0x1043144a0]error in unpack_block_qpis in 64 bit... [OK OS X 64 bit tho...]
  configure --disable-doc --disable-spec --disable-oggtest --disable-vorbistest --disable-examples --disable-asm

  make_install
)

(# build libsndfile, depends: libogg >= 1.1.3, libvorbis >= 1.2.3 for external support [disabled]. Uses dlfcn. 'build_libsndfile "install-libgsm"' to install the included LibGSM 6.10.
  download_and_unpack_file libsndfile https://github.com/libsndfile/libsndfile/releases/download/1.2.0/libsndfile-1.2.0.tar.xz

  # TODO: Enable external libs (will required building dlls for FLAC, Ogg and Vorbis)
  configure --disable-alsa --disable-sqlite --disable-external-libs --disable-full-suite

  make_install

  install -m644 src/GSM610/gsm.h $mingw_w64_x86_64_prefix/include/gsm.h || exit 1
  install -m644 src/GSM610/.libs/libgsm.a $mingw_w64_x86_64_prefix/lib/libgsm.a || exit 1
)

(# build lame, depends: dlfcn
  download_and_unpack_file lame https://sourceforge.net/projects/lame/files/lame/3.100/lame-3.100.tar.gz

  # Remove a UTF-8 BOM that breaks nasm if it's still there; should be fixed in trunk eventually https://sourceforge.net/p/lame/patches/81/
  sed -i.bak '1s/^\xEF\xBB\xBF//' libmp3lame/i386/nasm.h

  patch -p0 <"${script_path}/lame-3.100-sse-20171014.diff"
  patch -p0 <"${script_path}/patch-avoid_undefined_symbols_error.diff"

  configure --enable-nasm --disable-gtktest

  make_install
)

(# build twolame, depends: libsndfile >= 1.0.0 and dlfcn.
  download_and_unpack_file twolame https://github.com/njh/twolame/releases/download/0.4.0/twolame-0.4.0.tar.gz

  # Library only, front end refuses to build for some reason with git master
  sed -i.bak "/^SUBDIRS/s/ frontend.*//" Makefile.am || exit 1

  configure

  make_install
)

(# build libsoxr
  download_and_unpack_file https://downloads.sourceforge.net/project/soxr/soxr-0.1.3-Source.tar.xz

  cmake . -DHAVE_WORDS_BIGENDIAN_EXITCODE=0 -DWITH_OPENMP=0 -DBUILD_TESTS=0 -DBUILD_EXAMPLES=0

  make_install
)

(# build svt-av1
  download_and_unpack_file svt-av1 https://gitlab.com/AOMediaCodec/SVT-AV1/-/archive/v1.5.0/SVT-AV1-v1.5.0.tar.gz

  cd Build

  cmake .. -DCMAKE_BUILD_TYPE=Release -DCMAKE_SYSTEM_PROCESSOR=AMD64

  make_install
)

(# build fribidi, depends: dlfcn
  download_and_unpack_file fribidi https://github.com/fribidi/fribidi/releases/download/v1.0.13/fribidi-1.0.13.tar.xz

  configure --disable-debug --disable-deprecated --disable-docs

  make_install
)

(# build libxvid
  download_and_unpack_file xvidcore https://downloads.xvid.com/downloads/xvidcore-1.3.7.tar.gz

  cd build/generic

  # no static option...
  patch -p0 <"${script_path}/xvidcore-1.3.7_static-lib.patch"

  configure --host=$host_target --prefix=$mingw_w64_x86_64_prefix

  make install
)

(# build libvpx
  download_and_unpack_file libvpx https://chromium.googlesource.com/webm/libvpx/+archive/d6eb9696aa72473c1a11d34d928d35a3acc0c9a9.tar.gz

  # perhaps someday can remove this after 1.6.0 or mingw fixes it LOL
  patch -p1 <"${script_path}/vpx_160_semaphore.patch"

  export CHOST="$host_target"
  export CROSS="${host_target}-"
  # VP8 encoder *requires* sse3 support
  # fno for Error: invalid register for .seh_savexmm
  configure \
    --cpu=x86_64 \
    --target=x86_64-win64-gcc \
    --enable-ssse3 \
    --disable-examples \
    --disable-tools \
    --disable-docs \
    --disable-unit-tests \
    --enable-vp9-highbitdepth \
    --extra-cflags=-fno-asynchronous-unwind-tables \
    --extra-cflags=-mstackrealign

  make_install
)

(# build libx265
  download_and_unpack_file libx265 https://bitbucket.org/multicoreware/x265_git/downloads/x265_3.5.tar.gz

  mkdir -p 8bit 10bit 12bit

  # Build 12bit (main12)
  cd 12bit

  cmake ../source \
    -DMAIN12=1 \
    -DENABLE_CLI=0 \
    -DEXPORT_C_API=0 \
    -DENABLE_SHARED=0 \
    -DHIGH_BIT_DEPTH=1

  make

  cp libx265.a ../8bit/libx265_main12.a

  # Build 10bit (main10)
  cd ../10bit

  do_cmake_from_build_dir ../source \
    -DENABLE_CLI=0 \
    -DEXPORT_C_API=0 \
    -DENABLE_SHARED=0 \
    -DHIGH_BIT_DEPTH=1

  make

  cp libx265.a ../8bit/libx265_main10.a

  # Build 8 bit (main) with linked 10 and 12 bit then install
  cd ../8bit

  cmake ../source \
    -DEXTRA_LIB='x265_main10.a;x265_main12.a' \
    -DENABLE_CLI=1 \
    -DENABLE_SHARED=0 \
    -DLINKED_10BIT=TRUE \
    -DLINKED_12BIT=TRUE \
    -DEXTRA_LINK_FLAGS='-L .' \
    -DENABLE_HDR10_PLUS=1

  make

  mv libx265.a libx265_main.a

  "${host_target}-ar" -M <<EOF
CREATE libx265.a
ADDLIB libx265_main.a
ADDLIB libx265_main10.a
ADDLIB libx265_main12.a
SAVE
END
EOF

  make install
)

(# build libaom
  download_and_unpack_file libaom https://storage.googleapis.com/aom-releases/libaom-3.6.1.tar.gz

  mkdir -p aom_build

  cd aom_build

  cmake .. \
    -DCMAKE_TOOLCHAIN_FILE=../build/cmake/toolchains/x86_64-mingw-gcc.cmake \
    -DAOM_TARGET_CPU=x86_64

  make_install
)

(# build libx264
  download_and_unpack_file x264 https://code.videolan.org/videolan/x264/-/archive/baee400fa9ced6f5481a728138fed6e867b0ff7f/x264-baee400fa9ced6f5481a728138fed6e867b0ff7f.tar.gz

  # Change CFLAGS.
  sed -i "s/O3 -/O2 -/" configure

  # --enable-win32thread --enable-debug is another useful option here?
  set -- --cross-prefix="$cross_prefix" --enable-strip --disable-lavf --bit-depth=all
  for i in $CFLAGS; do
    # needs it this way seemingly :|
    set -- "$@" --extra-cflags="$i"
  done

  configure "$@"

  make_install
)

# TODO rav1e
# TODO libheif
# TODO OpenCL

(
  download_and_unpack_file ffmpeg https://ffmpeg.org/releases/ffmpeg-6.0.tar.xz

  ./configure \
    --cpu="x86_64" \
    --arch='x86_64' \
    --prefix="$mingw_w64_x86_64_prefix" \
    --target-os=mingw32 \
    --cross-prefix=$cross_prefix \
    --pkg-config=pkg-config \
    --pkg-config-flags=--static \
    --extra-libs='-lmpg123' \
    --extra-libs='-lshlwapi' \
    --extra-libs='-lpthread' \
    --extra-cflags=-DLIBTWOLAME_STATIC \
    --disable-alsa \
    --disable-cuda \
    --disable-cuvid \
    --disable-debug \
    --disable-doc \
    --disable-htmlpages \
    --disable-indevs \
    --disable-libjack \
    --disable-libopencore-amrnb \
    --disable-libopencore-amrwb \
    --disable-libpulse \
    --disable-libxcb \
    --disable-libxcb-shape \
    --disable-libxcb-shm \
    --disable-libxcb-xfixes \
    --disable-manpages \
    --disable-metal \
    --disable-neon-clobber-test \
    --disable-network \
    --disable-nvdec \
    --disable-nvenc \
    --disable-openssl \
    --disable-outdevs \
    --disable-podpages \
    --disable-postproc \
    --disable-programs \
    --disable-schannel \
    --disable-sdl2 \
    --disable-securetransport \
    --disable-sndio \
    --disable-static \
    --disable-txtpages \
    --disable-v4l2-m2m \
    --disable-vaapi \
    --disable-vdpau \
    --disable-vulkan \
    --disable-w32threads \
    --disable-xlib \
    --disable-xmm-clobber-test \
    --enable-avcodec \
    --enable-avfilter \
    --enable-avformat \
    --enable-avfoundation \
    --enable-bzlib \
    --enable-cross-compile \
    --enable-fontconfig \
    --enable-gpl \
    --enable-gray \
    --enable-iconv \
    --enable-inline-asm \
    --enable-libaom \
    --enable-libfreetype \
    --enable-libfribidi \
    --enable-libgsm \
    --enable-libmp3lame \
    --enable-libopenjpeg \
    --enable-libopus \
    --enable-librav1e \
    --enable-libsoxr \
    --enable-libsvtav1 \
    --enable-libtheora \
    --enable-libtwolame \
    --enable-libvorbis \
    --enable-libvpx \
    --enable-libwebp \
    --enable-libx264 \
    --enable-libx265 \
    --enable-libxml2 \
    --enable-libxvid \
    --enable-libzimg \
    --enable-lto \
    --enable-lzma \
    --enable-opencl \
    --enable-opengl \
    --enable-optimizations \
    --enable-pic \
    --enable-postproc \
    --enable-pthreads \
    --enable-shared \
    --enable-small \
    --enable-swscale \
    --enable-version3 \
    --enable-zlib

  make_install
)
