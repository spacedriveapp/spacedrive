#!/usr/bin/env bash

set -e          # exit immediate if an error occurs in a pipeline
set -E          # make commands inherit ERR trap
set -u          # don't allow not set variables to be utilized
set -o pipefail # trace ERR through pipes
set -o errtrace # trace ERR through 'time command' and other functions
set -x          # print to stderr each command before executing it

#---------------------------------- Constants ---------------------------------/
script_path="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd -P)"

export TRIPLE='x86_64-w64-mingw32'
export PREFIX="/srv/${TRIPLE}"

cross_prefix="$(dirname "$(command -v x86_64-w64-mingw32-gcc)")/${TRIPLE}-"

# Link system pkg-config to the cross one, meson requires it
ln -sfv "$(command -v pkg-config)" "${cross_prefix}pkg-config"

# Link dlltool to the cross one, libgit requires it
ln -sfv "${cross_prefix}dlltool" /usr/local/bin/dlltool

export AR="${cross_prefix}ar"
export LD="${cross_prefix}ld"
export CC="${cross_prefix}gcc"
export CXX="${cross_prefix}g++"
export STRIP="${cross_prefix}strip"
export RANLIB="${cross_prefix}ranlib"
# high compatible by default, see #219, some other good options are listed below, or you could use -march=native to target your local box:
export CFLAGS="-mtune=generic -O3 -I${PREFIX}/include"
export LDFLAGS="-L${PREFIX}/lib"
# Needed for mingw-w64 7 as FORTIFY_SOURCE is now partially implemented, but not actually working
export CPPFLAGS="-U_FORTIFY_SOURCE -D_FORTIFY_SOURCE=0 -I${PREFIX}/include"
export CXXFLAGS="$CPPFLAGS"
export PKG_CONFIG_PATH="${PREFIX}/lib/pkgconfig"
# disable pkg-config from finding [and using] normal linux system installed libs [yikes]
export PKG_CONFIG_LIBDIR=

export XZ_OPT='-T0 -9'

#---------------------------------- Functions ---------------------------------/
meson() {
  command meson setup \
    --strip \
    --prefix="${PREFIX}" \
    --libdir="${PREFIX}/lib" \
    --buildtype=release \
    --cross-file='/srv/meson-cross.mingw.txt' \
    --default-library=static \
    "$@" --unity=off
}

cmake() {
  local dir="$1"
  shift
  command cmake "$dir" \
    -G"Unix Makefiles" \
    -DCMAKE_RANLIB="${cross_prefix}ranlib" \
    -DCMAKE_LINKER="${cross_prefix}ld" \
    -DCMAKE_C_COMPILER="${cross_prefix}gcc" \
    -DBUILD_SHARED_LIBS="${SHARED:-0}" \
    -DCMAKE_SYSTEM_NAME=Windows \
    -DCMAKE_PREFIX_PATH="$PREFIX" \
    -DCMAKE_RC_COMPILER="${cross_prefix}windres" \
    -DCMAKE_CXX_COMPILER="${cross_prefix}g++" \
    -DCMAKE_INSTALL_PREFIX="$PREFIX" \
    -DCMAKE_FIND_ROOT_PATH="$PREFIX" \
    -DENABLE_STATIC_RUNTIME=1 \
    -DCMAKE_FIND_ROOT_PATH_MODE_LIBRARY=ONLY \
    -DCMAKE_FIND_ROOT_PATH_MODE_INCLUDE=ONLY \
    -DCMAKE_FIND_ROOT_PATH_MODE_PROGRAM=NEVER \
    "$@"
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
    --host="$TRIPLE" \
    --prefix="$PREFIX" \
    --with-sysroot="$(dirname "$(dirname "$cross_prefix")")" \
    --enable-static \
    --disable-shared \
    "$@"
}

fix_la() {
  find /srv/x86_64-w64-mingw32/lib -name '*.la' -exec sed -i 's@ =/@ /@g' {} +
}

make_install() {
  fix_la
  make "$@" -j"$(nproc)"
  make install "$@" -j"$(nproc)"
  fix_la
}

download_and_unpack_file() {
  if [ $# -lt 2 ]; then
    echo "download_and_unpack_file: Invalid arguments" >&2
    return 1
  fi

  local url="$2"

  mkdir -p "$1"

  cd "$1"

  if [ "${3:-}" != 'false' ]; then
    set -- --strip-components=1
  else
    set --
  fi

  curl -sSfL "$url" \
    | bsdtar -xf- "$@"
}

#------------------------------------ Main ------------------------------------/
mkdir -p "${PREFIX}/"{bin,include,lib,share}

CDPATH='' cd -- /srv

cat <<EOF >meson-cross.mingw.txt
[binaries]
c = '${cross_prefix}gcc'
cpp = '${cross_prefix}g++'
ld = '${cross_prefix}ld'
ar = '${cross_prefix}ar'
strip = '${cross_prefix}strip'
pkgconfig = '${cross_prefix}pkg-config'
nm = '${cross_prefix}nm'
windres = '${cross_prefix}windres'

[host_machine]
system = 'windows'
cpu_family = 'x86_64'
cpu = 'x86_64'
endian = 'little'
EOF

(# build mingw-std-threads
  download_and_unpack_file mingw-std-threads https://github.com/meganz/mingw-std-threads/archive/6c2061b.zip
  cp -av -- *.h "${PREFIX}/include"
)

(# Build dlfcn
  download_and_unpack_file dlfcn-win32 https://github.com/dlfcn-win32/dlfcn-win32/archive/6444294.zip

  # Change CFLAGS.
  sed -i "s/-O3/-O2/" Makefile

  chmod +x ./configure
  ./configure --prefix="$PREFIX" --cross-prefix="${TRIPLE}-"

  make_install

  # dlfcn-win32's 'README.md': "If you are linking to the static 'dl.lib' or 'libdl.a',
  # then you would need to explicitly add 'psapi.lib' or '-lpsapi' to your linking command,
  # depending on if MinGW is used."
  mv -f "${PREFIX}/lib/libdl.a" "${PREFIX}/lib/libdl_s.a"
  echo "GROUP ( -ldl_s -lpsapi )" >"${PREFIX}/lib/libdl.a"
)

(# build OpenCL Headers
  download_and_unpack_file opencl https://github.com/KhronosGroup/OpenCL-Headers/archive/refs/tags/v2023.04.17.tar.gz

  mkdir -p build

  cd build

  cmake ..

  make_install
)

(# build OpenCL icd
  download_and_unpack_file opencl-icd \
    https://github.com/KhronosGroup/OpenCL-ICD-Loader/archive/refs/tags/v2023.04.17.tar.gz

  mkdir -p build

  cd build

  cmake ..

  make_install
)

(# build zlib
  download_and_unpack_file zlib https://github.com/madler/zlib/releases/download/v1.2.13/zlib-1.2.13.tar.xz

  export CHOST="$TRIPLE"
  export ARFLAGS=rcs

  chmod +x ./configure
  ./configure --prefix="$PREFIX" --static

  make_install ARFLAGS="$ARFLAGS"
)

(# build bzip2
  download_and_unpack_file bzip2 https://sourceware.org/pub/bzip2/bzip2-1.0.8.tar.gz

  patch -p0 <"${script_path}/bzip2-1.0.8_brokenstuff.diff"

  make CC="$CC" AR="$AR" PREFIX="$PREFIX" RANLIB="$RANLIB" LD="$LD" STRIP="$STRIP" CXX="$CXX" libbz2.a -j"$(nproc)"

  install -m644 bzlib.h "${PREFIX}/include/bzlib.h"
  install -m644 libbz2.a "${PREFIX}/lib/libbz2.a"
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

(# build brotli
  download_and_unpack_file brotli https://github.com/google/brotli/archive/refs/tags/v1.0.9.tar.gz

  mkdir -p out

  cd out

  cmake .. -DBROTLI_DISABLE_TESTS=1 -DCMAKE_BUILD_TYPE=Release

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

  mkdir -p build

  cd build

  cmake .. -DBUILD_CODEC=0 -DCMAKE_BUILD_TYPE=Release

  make_install
)

(# build libpng, depends: zlib >= 1.0.4, dlfcn
  download_and_unpack_file libpng https://github.com/glennrp/libpng/archive/refs/tags/v1.6.39.tar.gz

  configure

  make_install
)

(# build libwebp, depends: dlfcn
  # Version 1.3.0
  download_and_unpack_file libwebp \
    https://chromium.googlesource.com/webm/libwebp.git/+archive/b557776962a3dcc985d83bd4ed94e1e2e50d0fa2.tar.gz \
    false

  export LIBPNG_CONFIG="${PREFIX}/bin/libpng-config --static" # LibPNG somehow doesn't get autodetected.

  configure --disable-wic

  make_install
)

(# build freetype, depends: bzip2
  download_and_unpack_file freetype \
    https://sourceforge.net/projects/freetype/files/freetype2/2.13.0/freetype-2.13.0.tar.xz

  # src/tools/apinames.c gets crosscompiled and makes the compilation fail
  patch -p0 <"${script_path}/freetype2-crosscompiled-apinames.diff"

  configure --with-bzip2 --without-harfbuzz

  make_install
)

(# build harfbuzz, depends: zlib, freetype, and libpng.
  download_and_unpack_file harfbuzz https://github.com/harfbuzz/harfbuzz/releases/download/7.3.0/harfbuzz-7.3.0.tar.xz

  mkdir -p build

  meson \
    -Dgdi=enabled \
    -Dicu=disabled \
    -Ddocs=disabled \
    -Dglib=disabled \
    -Dtests=disabled \
    -Dcairo=disabled \
    -Dtests=disabled \
    -Dgobject=disabled \
    -Dfreetype=enabled \
    -Dutilities=disabled \
    -Ddirectwrite=enabled \
    -Dintrospection=disabled \
    . build

  ninja -C build install
)

(# rebuild freetype with harfbuzz support
  cd /srv/freetype

  configure --with-bzip2 --with-harfbuzz

  make_install
)

(# build fontconfig, depends freetype, iconv and dlfcn.
  download_and_unpack_file fontconfig https://www.freedesktop.org/software/fontconfig/release/fontconfig-2.14.2.tar.xz

  meson \
    -Ddoc=disabled \
    -Dnls=disabled \
    -Dtests=disabled \
    -Dtools=disabled \
    -Dcache-build=disabled \
    . build

  ninja -C build install
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

  # disable asm: avoid [theora @ 0x1043144a0]error in unpack_block_qpis in 64 bit... [OK OS X 64 bit tho...]
  configure --disable-doc --disable-spec --disable-oggtest --disable-vorbistest --disable-examples --disable-asm

  make_install
)

(# build libsndfile, depends: dlfcn
  download_and_unpack_file libsndfile https://github.com/libsndfile/libsndfile/archive/refs/tags/1.2.0.tar.gz

  # TODO: Enable external libs (will required building dlls for FLAC, Ogg and Vorbis)
  configure --disable-alsa --disable-sqlite --disable-external-libs --disable-full-suite

  make_install

  install -m644 'src/GSM610/gsm.h' "${PREFIX}/include/gsm.h" || exit 1
  install -m644 'src/GSM610/.libs/libgsm.a' "${PREFIX}/lib/libgsm.a" || exit 1
)

(# build lame, depends: dlfcn
  download_and_unpack_file lame https://sourceforge.net/projects/lame/files/lame/3.100/lame-3.100.tar.gz

  # Remove a UTF-8 BOM that breaks nasm if it's still there; should be fixed in trunk eventually https://sourceforge.net/p/lame/patches/81/
  sed -i.bak '1s/^\xEF\xBB\xBF//' libmp3lame/i386/nasm.h

  patch -p0 <"${script_path}/lame-3.100-sse-20171014.diff"
  patch -p0 <"${script_path}/patch-avoid_undefined_symbols_error.diff"

  configure --enable-nasm --disable-gtktest --disable-frontend

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
  download_and_unpack_file soxr https://downloads.sourceforge.net/project/soxr/soxr-0.1.3-Source.tar.xz

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

  ./configure --host="$TRIPLE" --prefix="$PREFIX"

  make_install
)

(# build libvpx
  # v1.13.0
  download_and_unpack_file libvpx \
    https://chromium.googlesource.com/webm/libvpx/+archive/d6eb9696aa72473c1a11d34d928d35a3acc0c9a9.tar.gz false

  patch -p1 <"${script_path}/vpx_160_semaphore.patch"

  export CHOST="$TRIPLE"
  export CROSS="${TRIPLE}-"
  # VP8 encoder *requires* sse3 support
  # fno for Error: invalid register for .seh_savexmm
  ./configure \
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

(# build libx264
  # Stable
  download_and_unpack_file x264 \
    https://code.videolan.org/videolan/x264/-/archive/baee400fa9ced6f5481a728138fed6e867b0ff7f/x264-baee400fa9ced6f5481a728138fed6e867b0ff7f.tar.gz

  # Change CFLAGS.
  sed -i "s/O3 -/O2 -/" configure

  # --enable-win32thread --enable-debug is another useful option here?
  set -- --host="$TRIPLE" --enable-static --cross-prefix="$cross_prefix" --prefix="$PREFIX" --enable-strip --disable-lavf --bit-depth=all
  for i in $CFLAGS; do
    # needs it this way seemingly :|
    set -- "$@" --extra-cflags="$i"
  done

  ./configure "$@"

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

  cmake ../source \
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
    -DENABLE_CLI=0 \
    -DENABLE_SHARED=0 \
    -DLINKED_10BIT=TRUE \
    -DLINKED_12BIT=TRUE \
    -DEXTRA_LINK_FLAGS='-L .' \
    -DENABLE_HDR10_PLUS=1

  make

  mv libx265.a libx265_main.a

  "${TRIPLE}-ar" -M <<EOF
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

(# build rav1e
  download_and_unpack_file rav1e https://github.com/xiph/rav1e/archive/refs/tags/v0.6.6.tar.gz

  export TARGET="x86_64-pc-windows-gnu"
  export TARGET_CC="${cross_prefix}gcc"
  export TARGET_CXX="${cross_prefix}g++"
  export CROSS_COMPILE=1
  export TARGET_CFLAGS="$CFLAGS"
  export TARGET_CXXFLAGS="$CXXFLAGS"

  cat <<EOF >/root/.cargo/config.toml
[target.x86_64-pc-windows-gnu]
linker = "${LD}"
ar = "${AR}"
EOF

  cargo cinstall -v \
    --prefix="$PREFIX" \
    --target=x86_64-pc-windows-gnu \
    --release \
    --dlltool="${cross_prefix}dlltool" \
    --crt-static \
    --library-type=staticlib

  rm /root/.cargo/config.toml
)

(# build libheif
  download_and_unpack_file libheif https://github.com/strukturag/libheif/releases/download/v1.16.2/libheif-1.16.2.tar.gz

  while IFS= read -r -d '' file; do
    sed -i 's/#include <condition_variable>/#include "mingw.condition_variable.h"/g' "$file"
    sed -i 's/#include <future>/#include "mingw.future.h"/g' "$file"
    sed -i 's/#include <mutex>/#include "mingw.mutex.h"/g' "$file"
    sed -i 's/#include <shared_mutex>/#include "mingw.shared_mutex.h"/g' "$file"
    sed -i 's/#include <thread>/#include "mingw.thread.h"/g' "$file"
  done < <(find libheif -type f \( -name '*.cc' -o -name '*.h' \) -print0)

  mkdir -p build

  cd build

  export SHARED=1
  cmake .. \
    -DBUILD_TESTING=OFF \
    -DWITH_EXAMPLES=OFF \
    -DWITH_FUZZERS=OFF \
    -DWITH_REDUCED_VISIBILITY=ON \
    -DWITH_DEFLATE_HEADER_COMPRESSION=ON \
    -DWITH_AOM_DECODER_PLUGIN=OFF \
    -DWITH_AOM_ENCODER_PLUGIN=OFF \
    -DWITH_DAV1D_PLUGIN=OFF \
    -DWITH_LIBDE265_PLUGIN=OFF \
    -DWITH_RAV1E_PLUGIN=OFF \
    -DWITH_SvtEnc_PLUGIN=OFF \
    -DWITH_X265_PLUGIN=OFF

  make_install
)

(
  download_and_unpack_file ffmpeg https://ffmpeg.org/releases/ffmpeg-6.0.tar.xz

  ./configure \
    --cpu="x86_64" \
    --arch='x86_64' \
    --prefix="$PREFIX" \
    --sysroot="$(dirname "$(dirname "$cross_prefix")")" \
    --target-os=mingw32 \
    --cross-prefix="$cross_prefix" \
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
