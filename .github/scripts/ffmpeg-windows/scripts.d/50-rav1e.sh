#!/bin/bash

SCRIPT_REPO="https://github.com/xiph/rav1e.git"
SCRIPT_TAG="v0.6.6"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_TAG" rav1e
  cd rav1e

  local myconf=(
    --prefix=/opt/rav1e
    --library-type=cdylib
    --release
  )

  if [[ -n "$FFBUILD_RUST_TARGET" ]]; then
    unset PKG_CONFIG_LIBDIR

    export CC="gcc"
    export CXX="g++"
    export TARGET_CC="${FFBUILD_CROSS_PREFIX}gcc"
    export TARGET_CXX="${FFBUILD_CROSS_PREFIX}g++"
    export CROSS_COMPILE=1
    export TARGET_CFLAGS="$CFLAGS"
    export TARGET_CXXFLAGS="$CFLAGS"
    unset CFLAGS
    unset CXXFLAGS

    myconf+=(
      --target="$FFBUILD_RUST_TARGET"
    )
    cat <<EOF >$CARGO_HOME/config.toml
[target.$FFBUILD_RUST_TARGET]
linker = "${FFBUILD_CROSS_PREFIX}gcc"
ar = "${FFBUILD_CROSS_PREFIX}ar"
EOF
  fi

  cargo cinstall -v "${myconf[@]}"

  chmod 644 /opt/rav1e/lib/*rav1e*

  cp -nav /opt/rav1e/* "${FFBUILD_PREFIX}/"
  mkdir -p /opt/dlls/
  cp -nav /opt/rav1e/* /opt/dlls/
}
