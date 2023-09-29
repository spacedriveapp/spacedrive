#!/usr/bin/env bash

set -euo pipefail

case "${1:-}" in
  '' | x86_64-linux-gnu)
    export TARGET_TRIPLE='x86_64-linux-gnu'
    ;;
  aarch64-linux-gnu)
    export TARGET_TRIPLE='aarch64-linux-gnu'
    ;;
  *)
    echo "Unsupported target triple '${1}'"
    exit 1
    ;;
esac

# Change CWD to script dir
CDPATH='' cd "$(dirname "$0")"

echo "Install required build dependencies..."
apt-get update -yqq
apt-get install -yqq -o=Dpkg::Use-Pty=0 ninja-build cmake curl nasm pkg-config xz-utils patch python3

echo "Configure sysroot and prefix..."
mkdir -p "./src/prefix/bin" "./src/sysroot/bin"
_prefix="$(CDPATH='' cd ./src/prefix && pwd)"
_sysroot="$(CDPATH='' cd ./src/sysroot && pwd)"

# Configure PATH to use our sysroot bin
PATH="${_sysroot}/bin:$PATH"
export PATH

# Configure pkgconfig to look for our built libs
PKG_CONFIG_LIBDIR="${_prefix}/lib/pkgconfig:${_prefix}/share/pkgconfig"
export PKG_CONFIG_LIBDIR

# Download zig to use as a C/C++ cross compiler
echo "Download zig..."
curl -LSs "https://ziglang.org/download/0.11.0/zig-linux-$(uname -m)-0.11.0.tar.xz" \
  | tar -xJf- --strip-component 1 -C "$_sysroot"

mv "${_sysroot}/zig" "${_sysroot}/bin/zig"

# Create scripts for some zig internal commands, because cmake doesn't allow passing arguments to tools
for _arg in ar ranlib; do
  cat <<EOF >"${_sysroot}/bin/${_arg}"
#!/usr/bin/env bash
exec zig $_arg "\$@"
EOF
  chmod +x "${_sysroot}/bin/${_arg}"
done

echo "Download meson..."
mkdir -p ./src/meson

curl -LSs 'https://github.com/mesonbuild/meson/archive/refs/tags/1.2.1.tar.gz' \
  | tar -xzf- --strip-component 1 -C ./src/meson

pushd ./src/meson

# Patch meson to support zig as a C/C++ compiler
curl -LSs 'https://github.com/mesonbuild/meson/pull/12293.patch' | patch -p1
# Install meson binary
./packaging/create_zipapp.py --outfile "${_sysroot}/bin/meson" --compress

popd

cat <<EOF >./src/cross.meson
[binaries]
c = ['zig', 'cc', '-s', '-target', '$TARGET_TRIPLE']
cpp = ['zig', 'c++', '-s', '-target', '$TARGET_TRIPLE']
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

if [ "$TARGET_TRIPLE" = 'aarch64-linux-gnu' ]; then
  cat <<EOF >>./src/cross.meson

[target_machine]
system = 'linux'
cpu_family = 'aarch64'
cpu = 'arm64'
endian = 'little'
EOF
fi

cat <<EOF >./src/toolchain.cmake
set(CMAKE_SYSTEM_NAME Linux)
set(CMAKE_SYSTEM_PROCESSOR x86_64)

set(triple $TARGET_TRIPLE)

set(CMAKE_CROSSCOMPILING TRUE)
set_property(GLOBAL PROPERTY TARGET_SUPPORTS_SHARED_LIBS FALSE)

# Do a no-op access on the CMAKE_TOOLCHAIN_FILE variable so that CMake will not
# issue a warning on it being unused.
if (CMAKE_TOOLCHAIN_FILE)
endif()

set(CMAKE_C_COMPILER zig cc -s -target $TARGET_TRIPLE)
set(CMAKE_CXX_COMPILER zig c++ -s -target $TARGET_TRIPLE)
set(CMAKE_RANLIB ranlib)
set(CMAKE_C_COMPILER_RANLIB ranlib)
set(CMAKE_CXX_COMPILER_RANLIB ranlib)
set(CMAKE_AR ar)
set(CMAKE_C_COMPILER_AR ar)
set(CMAKE_CXX_COMPILER_AR ar)

set(CMAKE_FIND_ROOT_PATH ${_prefix} ${_sysroot})
set(CMAKE_SYSTEM_PREFIX_PATH /)

if(CMAKE_INSTALL_PREFIX_INITIALIZED_TO_DEFAULT)
  set(CMAKE_INSTALL_PREFIX "${_prefix}" CACHE PATH
    "Install path prefix, prepended onto install directories." FORCE)
endif()

# To find programs to execute during CMake run time with find_program(), e.g.
# 'git' or so, we allow looking into system paths.
set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)

if (NOT CMAKE_FIND_ROOT_PATH_MODE_LIBRARY)
  set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
endif()
if (NOT CMAKE_FIND_ROOT_PATH_MODE_INCLUDE)
  set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
endif()
if (NOT CMAKE_FIND_ROOT_PATH_MODE_PACKAGE)
  set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)
endif()

# TODO: CMake appends <sysroot>/usr/include to implicit includes; switching to use usr/include will make this redundant.
if ("\${CMAKE_C_IMPLICIT_INCLUDE_DIRECTORIES}" STREQUAL "")
  set(CMAKE_C_IMPLICIT_INCLUDE_DIRECTORIES "${_prefix}/include")
endif()
if ("\${CMAKE_CXX_IMPLICIT_INCLUDE_DIRECTORIES}" STREQUAL "")
  set(CMAKE_CXX_IMPLICIT_INCLUDE_DIRECTORIES "${_prefix}/include")
endif()
EOF

# --

echo "Download zlib..."
mkdir -p ./src/zlib/build

curl -LSs 'https://github.com/madler/zlib/archive/refs/tags/v1.3.tar.gz' \
  | tar -xzf- --strip-component 1 -C ./src/zlib

pushd ./src/zlib/build

echo "Build zlib..."
cmake \
  -GNinja \
  -DCMAKE_TOOLCHAIN_FILE=../../toolchain.cmake \
  -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_SHARED_LIBS=Off \
  -DCMAKE_POSITION_INDEPENDENT_CODE=On \
  -DCMAKE_SKIP_INSTALL_ALL_DEPENDENCY=On \
  -DCMAKE_INSTALL_PREFIX="$_prefix" \
  ..
ninja -j"$(nproc)" zlibstatic
# Stub .so files so install doesn't fail
touch libz.so.1.3 libz.so.1 libz.so
ninja install

# Remove stub .so files
rm "${_prefix}"/lib/{libz.so.1.3,libz.so.1,libz.so}

popd

# --

echo "Download dav1d..."
mkdir -p ./src/dav1d/build

curl -LSs 'https://code.videolan.org/videolan/dav1d/-/archive/1.2.1/dav1d-1.2.1.tar.gz' \
  | tar -xzf- --strip-component 1 -C ./src/dav1d

pushd ./src/dav1d/build

echo "Build dav1d..."
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

# --

echo "Download libde265..."
mkdir -p ./src/libde265/build

curl -#LSs 'https://github.com/strukturag/libde265/archive/refs/tags/v1.0.12.tar.gz' \
  | tar -xzf- --strip-component 1 -C ./src/libde265

pushd ./src/libde265/build

echo "Build libde265..."
cmake \
  -GNinja \
  -DCMAKE_TOOLCHAIN_FILE=../../toolchain.cmake \
  -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_SHARED_LIBS=Off \
  -DCMAKE_POSITION_INDEPENDENT_CODE=On \
  -DCMAKE_INSTALL_PREFIX="$_prefix" \
  -DENABLE_SDL=Off \
  -DENABLE_DECODER=Off \
  -DENABLE_ENCODER=Off \
  ..
ninja -j"$(nproc)"
ninja install

popd

# --

echo "Download libwebp..."
mkdir -p ./src/libwebp/build

curl -#LSs 'https://github.com/webmproject/libwebp/archive/refs/tags/v1.3.2.tar.gz' \
  | tar -xzf- --strip-component 1 -C ./src/libwebp

pushd ./src/libwebp/build

echo "Build libwebp..."
cmake \
  -GNinja \
  -DCMAKE_TOOLCHAIN_FILE=../../toolchain.cmake \
  -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_SHARED_LIBS=Off \
  -DCMAKE_POSITION_INDEPENDENT_CODE=On \
  -DCMAKE_INSTALL_PREFIX="$_prefix" \
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

# --

echo "Download libheif..."
mkdir -p ./src/libheif/build

curl -#LSs 'https://github.com/strukturag/libheif/archive/refs/tags/v1.16.2.tar.gz' \
  | tar -xzf- --strip-component 1 -C ./src/libheif

pushd ./src/libheif/build

echo "Build libheif..."
cmake \
  -GNinja \
  -DCMAKE_TOOLCHAIN_FILE=../../toolchain.cmake \
  -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_SHARED_LIBS=Off \
  -DCMAKE_POSITION_INDEPENDENT_CODE=On \
  -DCMAKE_INSTALL_PREFIX="$_prefix" \
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
