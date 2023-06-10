#!/bin/bash

SCRIPT_REPO="git://sourceware.org/git/bzip2.git"
SCRIPT_REF="bzip2-1.0.8"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_REF" bzip2

  cd bzip2

  patch -p0 </patches/bzip2-1.0.8_brokenstuff.diff

  make \
    CC="${FFBUILD_CROSS_PREFIX}gcc" \
    AR="${FFBUILD_CROSS_PREFIX}ar" \
    LD="${FFBUILD_CROSS_PREFIX}ld" \
    CXX="${FFBUILD_CROSS_PREFIX}g++" \
    STRIP="${FFBUILD_CROSS_PREFIX}strip" \
    PREFIX="$FFBUILD_PREFIX" \
    RANLIB="${FFBUILD_CROSS_PREFIX}ranlib" \
    libbz2.a \
    -j"$(nproc)"

  install -m644 -D bzlib.h "${FFBUILD_PREFIX}/include/bzlib.h"
  install -m644 -D libbz2.a "${FFBUILD_PREFIX}/lib/libbz2.a"
}
