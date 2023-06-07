#!/bin/bash

SCRIPT_REPO="https://github.com/FFmpeg/nv-codec-headers.git"
SCRIPT_COMMIT="4fd7be29a431441ca31b8db0155875ae2ff4ed47"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_COMMIT" ffnvcodec
  cd ffnvcodec

  make PREFIX="$FFBUILD_PREFIX" install
}
