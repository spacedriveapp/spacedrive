#!/bin/bash

SCRIPT_REPO="https://svn.code.sf.net/p/lame/svn/trunk/lame"
SCRIPT_REV="6507"

ffbuild_dockerbuild() {
  retry-tool sh -c "rm -rf lame && svn checkout '${SCRIPT_REPO}@${SCRIPT_REV}' lame"
  cd lame

  autoreconf -i

  local myconf=(
    --prefix="$FFBUILD_PREFIX"
    --disable-shared
    --enable-static
    --enable-nasm
    --disable-gtktest
    --disable-cpml
    --disable-frontend
    --disable-decoder
  )

  if [[ $TARGET == win* || $TARGET == linux* ]]; then
    myconf+=(
      --host="$FFBUILD_TOOLCHAIN"
    )
  else
    echo "Unknown target"
    return 255
  fi

  export CFLAGS="$CFLAGS -DNDEBUG"

  ./configure "${myconf[@]}"
  make -j"$(nproc)"
  make install
}
