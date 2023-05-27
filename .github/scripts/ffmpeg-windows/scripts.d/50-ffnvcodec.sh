#!/bin/bash

SCRIPT_REPO="https://github.com/FFmpeg/nv-codec-headers.git"
SCRIPT_COMMIT="450f8a634309b8f212b1a642630e164150d6a427"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_COMMIT" ffnvcodec
  cd ffnvcodec

  make PREFIX="$FFBUILD_PREFIX" install
}
