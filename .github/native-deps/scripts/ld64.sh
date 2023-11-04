#!/usr/bin/env bash

set -euo pipefail

argv=(
  -macos_version_min "${MACOSX_DEPLOYMENT_TARGET?Missing macOS deployment target version}"
  -platform_version macos "$MACOSX_DEPLOYMENT_TARGET" "${MACOS_SDK_VERSION:?Missing macOS SDK version}"
  -S
)

while [ "$#" -gt 0 ]; do
  if [ "$1" = '-macosx_version_min' ] || [ "$1" = '-macos_version_min' ]; then
    shift
  elif [ "$1" = '-S' ]; then
    true
  else
    argv+=("$1")
  fi

  shift
done

exec ld64.lld-16 "${argv[@]}"
