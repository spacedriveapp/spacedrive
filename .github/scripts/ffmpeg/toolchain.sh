#!/usr/bin/env bash

set -euo pipefail

if [ -z "${TARGET:-}" ]; then
  echo "Missing TARGET envvar" >&2
  exit 1
fi

# The the target system name (*-middle-*) from the target triple
SYSTEM_NAME="${TARGET#*-}"
SYSTEM_NAME="${SYSTEM_NAME%-*}"

# On windows this should be AMD64 or ARM64, but most cmake scripts don't check against the windows specific names
# As most of this stack is clang based, we can just use the POSIX names which should be fine
SYSTEM_PROCESSOR="${TARGET%%-*}"

if ! [ -d "${SYSROOT:-}" ]; then
  echo "Invalid sysroot provided: ${2:-}" >&2
  exit 1
fi

if ! [ -d "${PREFIX:-}" ]; then
  echo "Invalid prefix provided: ${3:-}" >&2
  exit 1
fi

cat <<EOF >/srv/cross.meson
[binaries]
c = ['zig-cc']
ar = ['zig', 'ar']
cpp = ['zig-c++']
lib = ['zig', 'lib']
strip = ['llvm-strip-17']
ranlib = ['zig', 'ranlib']
windres = ['rc']
dlltool = ['zig', 'dlltool']
objcopy = [ 'zig', 'objcopy' ]
objdump = [ 'llvm-objdump-17' ]
readelf = [ 'llvm-readelf-17' ]

[properties]
sys_root = '${SYSROOT}'
cmake_defaults = false
pkg_config_libdir = ['${PREFIX}/lib/pkgconfig', '${PREFIX}/share/pkgconfig']
cmake_toolchain_file = '/srv/toolchain.cmake'

[host_machine]
cpu = '${SYSTEM_PROCESSOR}'
endian = 'little'
system = '${SYSTEM_NAME}'
cpu_family = '${SYSTEM_PROCESSOR}'

EOF

cat <<EOF >/srv/toolchain.cmake
set(CMAKE_SYSTEM_NAME ${SYSTEM_NAME^})
set(CMAKE_SYSTEM_PROCESSOR $SYSTEM_PROCESSOR)

set(CMAKE_CROSSCOMPILING TRUE)

# Do a no-op access on the CMAKE_TOOLCHAIN_FILE variable so that CMake will not
# issue a warning on it being unused.
if (CMAKE_TOOLCHAIN_FILE)
endif()

set(CMAKE_C_COMPILER zig-cc)
set(CMAKE_CXX_COMPILER zig-c++)
set(CMAKE_RANLIB ranlib)
set(CMAKE_C_COMPILER_RANLIB ranlib)
set(CMAKE_CXX_COMPILER_RANLIB ranlib)
set(CMAKE_AR ar)
set(CMAKE_OBJCOPY objcopy)
set(CMAKE_OBJDUMP llvm-objdump-17)
set(CMAKE_READELF llvm-readelf-17)
set(CMAKE_C_COMPILER_AR ar)
set(CMAKE_CXX_COMPILER_AR ar)
set(CMAKE_RC_COMPILER rc)

set(CMAKE_FIND_ROOT_PATH ${PREFIX} ${SYSROOT})
set(CMAKE_SYSTEM_PREFIX_PATH /)

if(CMAKE_INSTALL_PREFIX_INITIALIZED_TO_DEFAULT)
  set(CMAKE_INSTALL_PREFIX "${PREFIX}" CACHE PATH
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
  set(CMAKE_C_IMPLICIT_INCLUDE_DIRECTORIES ${PREFIX}/include)
endif()
if ("\${CMAKE_CXX_IMPLICIT_INCLUDE_DIRECTORIES}" STREQUAL "")
  set(CMAKE_CXX_IMPLICIT_INCLUDE_DIRECTORIES ${PREFIX}/include)
endif()
EOF

mkdir -p "${PREFIX}/lib/pkgconfig"

# Zig has internal support for libunwind
cat <<EOF >"${PREFIX}/lib/pkgconfig/unwind.pc"
prefix=${SYSROOT}/lib/libunwind
includedir=\${prefix}/include

Name: Libunwind
Description: Zig has internal support for libunwind
Version: 9999
Cflags: -I\${includedir}
Libs: -lunwind
EOF

ln -s "unwind.pc" "${PREFIX}/lib/pkgconfig/libunwind.pc"

# Replace libgcc_s with libunwind
ln -s "unwind.pc" "${PREFIX}/lib/pkgconfig/gcc_s.pc"
ln -s "unwind.pc" "${PREFIX}/lib/pkgconfig/libgcc_s.pc"

# zig doesn't provide libgcc_eh
# As an alternative use libc++ to replace it on windows gnu targets
cat <<EOF >"${PREFIX}/lib/pkgconfig/gcc_eh.pc"
Name: libgcc_eh
Description: Replace libgcc_eh with libc++
Version: 9999
Libs.private: -lc++
EOF

ln -s "gcc_eh.pc.pc" "${PREFIX}/lib/pkgconfig/libgcc_eh.pc"

case "$TARGET" in
  *windows-gnu)
    # Work around LTO bugs when compiling C++ for windows targets
    # https://github.com/ziglang/zig/issues/15958#issuecomment-1764915440
    sed -i '/_free_locale))(_locale_t)/s/^/__attribute__((used)) /' "${SYSROOT}/lib/libc/mingw/misc/_free_locale.c"
    sed -i '/_create_locale))(int, const char \*)/s/^/__attribute__((used)) /' "${SYSROOT}/lib/libc/mingw/misc/_create_locale.c"
    ;;
esac
