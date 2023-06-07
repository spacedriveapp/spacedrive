#!/bin/bash

SCRIPT_REPO="https://git.code.sf.net/p/mingw-w64/mingw-w64.git"
SCRIPT_COMMIT="9df2e604ddf16765410724716a8d1887ffc61fa9"

ffbuild_dockerbuild() {
  retry-tool sh -c "rm -rf mingw && git clone '$SCRIPT_REPO' mingw"
  cd mingw
  git checkout "$SCRIPT_COMMIT"

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
    --with-default-msvcrt=ucrt
    --enable-idl
  )

  ./configure "${myconf[@]}"
  make -j"$(nproc)"
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
  make -j"$(nproc)"
  make install DESTDIR="/opt/mingw"
}
