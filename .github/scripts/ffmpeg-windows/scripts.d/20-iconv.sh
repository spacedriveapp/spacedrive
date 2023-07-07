#!/bin/bash

SCRIPT_REPO="https://git.savannah.gnu.org/git/libiconv.git"
SCRIPT_TAG="v1.17"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" libiconv
  cd libiconv

  retry-tool ./gitsub.sh pull
  (unset CC CFLAGS GMAKE && ./autogen.sh)

  local myconf=(
    --host="$FFBUILD_TOOLCHAIN"
    --prefix="$FFBUILD_PREFIX"
    --enable-extra-encodings
    --disable-shared
    --enable-static
    --with-pic
  )

  ./configure "${myconf[@]}"
  make -j"$(nproc)"
  make install
}
