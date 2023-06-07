#!/bin/bash

SCRIPT_REPO="https://github.com/GPUOpen-LibrariesAndSDKs/AMF.git"
SCRIPT_COMMIT="bd5db31d3d8ea1fae7002dd903898599bdb7d97a"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_COMMIT" amf
  cd amf

  mkdir -p "$FFBUILD_PREFIX"/include
  mv amf/public/include "$FFBUILD_PREFIX"/include/AMF
}
