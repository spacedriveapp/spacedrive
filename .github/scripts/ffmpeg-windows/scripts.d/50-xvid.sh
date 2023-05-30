#!/bin/bash

SCRIPT_REPO="http://svn.xvid.org/trunk/xvidcore"
SCRIPT_REV="2198"

ffbuild_dockerbuild() {
  retry-tool sh -c "rm -rf xvid && svn checkout --username 'anonymous' --password '' '${SCRIPT_REPO}@${SCRIPT_REV}' xvid"
  cd xvid

  cd build/generic

  # The original code fails on a two-digit major...
  sed -i -e 's/GCC_MAJOR=.*/GCC_MAJOR=10/' \
    -e 's/GCC_MINOR=.*/GCC_MINOR=0/' \
    configure.in

  ./bootstrap.sh

  local myconf=(
    --prefix="$FFBUILD_PREFIX"
  )

  if [[ $TARGET == win* || $TARGET == linux* ]]; then
    myconf+=(
      --host="$FFBUILD_TOOLCHAIN"
    )
  else
    echo "Unknown target"
    return -1
  fi

  ./configure "${myconf[@]}"
  make -j"$(nproc)"
  make install

  if [[ $TARGET == win* ]]; then
    rm "$FFBUILD_PREFIX"/{bin/libxvidcore.dll,lib/libxvidcore.dll.a}
  elif [[ $TARGET == linux* ]]; then
    rm "$FFBUILD_PREFIX"/lib/libxvidcore.so*
  fi
}
