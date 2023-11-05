#!/usr/bin/env bash

set -xeuo pipefail

case "$TARGET" in
  *darwin*)
    curl_tar \
      "https://github.com/joseluisq/macosx-sdks/releases/download/${MACOS_SDK_VERSION:?}/MacOSX${MACOS_SDK_VERSION:?}.sdk.tar.xz" \
      "${MACOS_SDKROOT:?}" 1
    ;;
esac
