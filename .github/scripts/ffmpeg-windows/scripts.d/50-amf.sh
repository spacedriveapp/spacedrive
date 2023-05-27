#!/bin/bash

SCRIPT_REPO="https://github.com/GPUOpen-LibrariesAndSDKs/AMF.git"
SCRIPT_COMMIT="daee84bf3cde928dc1cd0b502c1b6a50cbff2495"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_COMMIT" amf
  cd amf

  mkdir -p "$FFBUILD_PREFIX"/include
  mv amf/public/include "$FFBUILD_PREFIX"/include/AMF
}
