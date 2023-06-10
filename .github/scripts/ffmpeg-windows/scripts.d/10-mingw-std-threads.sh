#!/bin/bash

SCRIPT_REPO="https://github.com/meganz/mingw-std-threads.git"
SCRIPT_COMMIT="6c2061b7da41d6aa1b2162ff4383ec3ece864bc6"

ffbuild_dockerbuild() {
  git-mini-clone "$SCRIPT_REPO" "$SCRIPT_COMMIT" mingw-std-threads
  cd mingw-std-threads

  mkdir -p "$FFBUILD_PREFIX"/include
  cp *.h "$FFBUILD_PREFIX"/include
}
