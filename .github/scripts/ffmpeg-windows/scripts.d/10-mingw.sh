#!/bin/bash

SCRIPT_REPO="https://github.com/mirror/mingw-w64.git"
SCRIPT_COMMIT="eff726c461e09f35eeaed125a3570fa5f807f02b"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_COMMIT" mingw
  cd mingw

  cd mingw-w64-headers

  unset CFLAGS
  unset CXXFLAGS
  unset LDFLAGS
  unset PKG_CONFIG_LIBDIR

  GCC_SYSROOT="$(${FFBUILD_CROSS_PREFIX}gcc -print-sysroot)"

  local myconf=(
    --prefix="$GCC_SYSROOT/usr/$FFBUILD_TOOLCHAIN"
    --host="$FFBUILD_TOOLCHAIN"
    --with-default-win32-winnt="0x601"
    --enable-idl
  )

  ./configure "${myconf[@]}"
  make -j$(nproc)
  make install DESTDIR="/opt/mingw"

  cd ../mingw-w64-libraries/winpthreads

  local myconf=(
    --prefix="$GCC_SYSROOT/usr/$FFBUILD_TOOLCHAIN"
    --host="$FFBUILD_TOOLCHAIN"
    --with-pic
    --disable-shared
    --enable-static
  )

  ./configure "${myconf[@]}"
  make -j$(nproc)
  make install DESTDIR="/opt/mingw"
}
