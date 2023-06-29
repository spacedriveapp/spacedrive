#!/bin/bash

SCRIPT_REPO="https://github.com/zapping-vbi/zvbi.git"
SCRIPT_TAG="v0.2.41"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" zvbi
  cd zvbi

  (
    set -- \
    'https://github.com/videolan/vlc/raw/f7bb59d9f51cc10b25ff86d34a3eff744e60c46e/contrib/src/zvbi/zvbi-ssize_max.patch' \
    'https://github.com/videolan/vlc/raw/f7bb59d9f51cc10b25ff86d34a3eff744e60c46e/contrib/src/zvbi/zvbi-ioctl.patch'
    # 'https://github.com/videolan/vlc/raw/f7bb59d9f51cc10b25ff86d34a3eff744e60c46e/contrib/src/zvbi/zvbi-fix-static-linking.patch'

    if [[ $TARGET == win* ]]; then
      set -- "$@" \
        'https://github.com/videolan/vlc/raw/f7bb59d9f51cc10b25ff86d34a3eff744e60c46e/contrib/src/zvbi/zvbi-win32.patch' \
        'https://github.com/videolan/vlc/raw/f7bb59d9f51cc10b25ff86d34a3eff744e60c46e/contrib/src/zvbi/zvbi-win32-undefined.patch'
    fi

    set -- "$@" \
      'https://github.com/videolan/vlc/raw/f7bb59d9f51cc10b25ff86d34a3eff744e60c46e/contrib/src/zvbi/zvbi-fix-clang-support.patch'

    for path in "$@"; do
      curl -LSs "$path" | patch -p1
    done
  )

  ./autogen.sh

  local myconf=(
    --host="$FFBUILD_TOOLCHAIN"
    --prefix="$FFBUILD_PREFIX"
    --disable-shared
    --enable-static
    --with-pic
    --without-doxygen
    --without-x
    --disable-dvb
    --disable-bktr
    --disable-nls
    --disable-v4l
    --disable-proxy
  )

  ./configure "${myconf[@]}"

  make -j"$(nproc)" install

  sed -i "s/\/[^ ]*libiconv.a/-liconv/" "$FFBUILD_PREFIX"/lib/pkgconfig/zvbi-0.2.pc
}
