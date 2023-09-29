#!/usr/bin/env bash

set -euxo pipefail

# Change CWD to script dir
CDPATH='' cd "$(dirname "$0")"

apt-get -y update
apt-get -y install ninja-build cmake curl nasm pkg-config xz-utils patch python3

PKG_CONFIG_LIBDIR="$(realpath -m ./src/prefix/lib/pkgconfig):$(realpath -m ./src/prefix/share/pkgconfig)"
export PKG_CONFIG_LIBDIR

mkdir -p ./src/prefix/bin

_prefix="$(realpath ./src/prefix)"

curl -LSs 'https://ziglang.org/download/0.11.0/zig-linux-x86_64-0.11.0.tar.xz' \
  | tar -xJf- --strip-component 1 -C ./src/prefix

mv ./src/prefix/zig ./src/prefix/bin/zig

for _arg in ar ranlib; do
  cat <<EOF >./src/prefix/bin/$_arg
#!/usr/bin/env bash
exec zig $_arg "\$@"
EOF
  chmod +x ./src/prefix/bin/$_arg
done

PATH="${_prefix}/bin:$PATH"
export PATH

mkdir -p ./src/meson

curl -LSs 'https://github.com/mesonbuild/meson/archive/refs/tags/1.2.1.tar.gz' \
  | tar -xzf- --strip-component 1 -C ./src/meson

pushd ./src/meson

curl -LSs 'https://github.com/mesonbuild/meson/pull/12293.patch' | patch -p1

./packaging/create_zipapp.py --outfile ../prefix/bin/meson --compress

popd

cat <<EOF >./src/cross.meson
[binaries]
c = ['zig', 'cc']
cpp = ['zig', 'c++']
ar = ['zig', 'ar']
ranlib = ['zig', 'ranlib']
lib = ['zig', 'lib']
dlltool = ['zig', 'dlltool']

[host_machine]
system = 'linux'
cpu_family = 'x86_64'
cpu = 'x86_64'
endian = 'little'
EOF

cat <<EOF >./src/toolchain.cmake
set(CMAKE_SYSTEM_NAME Linux)
set(CMAKE_SYSTEM_PROCESSOR x86_64)

set(triple x86_64-linux-gnu)

set(CMAKE_C_COMPILER zig cc)
set(CMAKE_CXX_COMPILER zig c++)
set(CMAKE_RANLIB ranlib)
set(CMAKE_C_COMPILER_RANLIB ranlib)
set(CMAKE_AR ar)
set(CMAKE_C_COMPILER_AR ar)

set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
EOF

mkdir -p ./src/zlib/build

curl -LSs 'https://github.com/madler/zlib/archive/refs/tags/v1.3.tar.gz' \
  | tar -xzf- --strip-component 1 -C ./src/zlib

pushd ./src/zlib/build

cmake \
  -GNinja \
  -DCMAKE_TOOLCHAIN_FILE=../../toolchain.cmake \
  -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_SHARED_LIBS=Off \
  -DCMAKE_POSITION_INDEPENDENT_CODE=On \
	-DCMAKE_SKIP_INSTALL_ALL_DEPENDENCY=On \
  -DCMAKE_INSTALL_PREFIX="$_prefix" \
  -DCMAKE_INSTALL_BINDIR="$_prefix"/bin \
  -DCMAKE_INSTALL_LIBDIR="$_prefix"/lib \
  ..
ninja -j"$(nproc)" zlibstatic
touch libz.so.1.3 libz.so.1 libz.so
ninja install

rm /src/prefix/lib/{libz.so.1.3,libz.so.1,libz.so}

popd

mkdir -p ./src/dav1d/build

curl -LSs 'https://code.videolan.org/videolan/dav1d/-/archive/1.2.1/dav1d-1.2.1.tar.gz' \
  | tar -xzf- --strip-component 1 -C ./src/dav1d

pushd ./src/dav1d/build

meson setup \
  --cross-file=../../cross.meson \
  -Denable_docs=false \
  -Denable_tools=false \
  -Denable_tests=false \
  -Denable_examples=false \
  --prefix="$_prefix" \
  --buildtype=release \
  --default-library=static \
  ..
ninja -j"$(nproc)"
ninja install

popd

mkdir -p ./src/libde265/build

curl -#LSs 'https://github.com/strukturag/libde265/archive/refs/tags/v1.0.12.tar.gz' \
  | tar -xzf- --strip-component 1 -C ./src/libde265

pushd ./src/libde265/build

cmake \
  -GNinja \
  -DCMAKE_TOOLCHAIN_FILE=../../toolchain.cmake \
  -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_SHARED_LIBS=Off \
  -DCMAKE_POSITION_INDEPENDENT_CODE=On \
  -DCMAKE_INSTALL_PREFIX="$_prefix" \
  -DCMAKE_INSTALL_BINDIR="$_prefix"/bin \
  -DCMAKE_INSTALL_LIBDIR="$_prefix"/lib \
  -DENABLE_SDL=Off \
  -DENABLE_DECODER=Off \
  -DENABLE_ENCODER=Off \
  ..
ninja -j"$(nproc)"
ninja install

popd

mkdir -p ./src/libwebp/build

curl -#LSs 'https://github.com/webmproject/libwebp/archive/refs/tags/v1.3.2.tar.gz' \
  | tar -xzf- --strip-component 1 -C ./src/libwebp

pushd ./src/libwebp/build

cmake \
  -GNinja \
  -DCMAKE_TOOLCHAIN_FILE=../../toolchain.cmake \
  -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_SHARED_LIBS=Off \
  -DCMAKE_POSITION_INDEPENDENT_CODE=On \
  -DCMAKE_INSTALL_PREFIX="$_prefix" \
  -DCMAKE_INSTALL_BINDIR="$_prefix"/bin \
  -DCMAKE_INSTALL_LIBDIR="$_prefix"/lib \
  -DWEBP_LINK_STATIC=On \
  -DWEBP_BUILD_CWEBP=Off \
  -DWEBP_BUILD_DWEBP=Off \
  -DWEBP_BUILD_GIF2WEBP=Off \
  -DWEBP_BUILD_IMG2WEBP=Off \
  -DWEBP_BUILD_VWEBP=Off \
  -DWEBP_BUILD_WEBPINFO=Off \
  -DWEBP_BUILD_WEBPMUX=Off \
  -DWEBP_BUILD_EXTRAS=Off \
  -DWEBP_BUILD_ANIM_UTILS=Off \
  ..
ninja -j"$(nproc)"
ninja install

popd

mkdir -p ./src/libheif/build

curl -#LSs 'https://github.com/strukturag/libheif/archive/refs/tags/v1.16.2.tar.gz' \
  | tar -xzf- --strip-component 1 -C ./src/libheif

pushd ./src/libheif/build

cmake \
  -GNinja \
  -DCMAKE_TOOLCHAIN_FILE=../../toolchain.cmake \
  -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_SHARED_LIBS=Off \
  -DCMAKE_POSITION_INDEPENDENT_CODE=On \
  -DCMAKE_INSTALL_PREFIX="$_prefix" \
  -DCMAKE_INSTALL_BINDIR="$_prefix"/bin \
  -DCMAKE_INSTALL_LIBDIR="$_prefix"/lib \
  -DBUILD_TESTING=OFF \
  -DWITH_DAV1D=ON \
  -DWITH_DAV1D_PLUGIN=OFF \
  -DWITH_LIBDE265=ON \
  -DWITH_LIBDE265_PLUGIN=OFF \
  -DWITH_LIBSHARPYUV=ON \
  -DWITH_FUZZERS=OFF \
  -DWITH_EXAMPLES=OFF \
  -DWITH_UNCOMPRESSED_CODEC=ON \
  -DWITH_REDUCED_VISIBILITY=ON \
  -DWITH_DEFLATE_HEADER_COMPRESSION=ON \
  -DENABLE_PLUGIN_LOADING=OFF \
  -DENABLE_MULTITHREADING_SUPPORT=ON \
  ..
ninja -j"$(nproc)"
ninja install

popd
