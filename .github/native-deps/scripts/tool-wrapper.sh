#!/usr/bin/env bash

set -euo pipefail

fallback_llvm() {
  if ! command -v "${_prefix}$(basename "$0")" 1>/dev/null 2>&1; then
    _prefix='llvm-'
    if [ "$0" = 'libtool' ]; then
      _suffix='-darwin-16'
    else
      _suffix='-16'
    fi
  fi
}

case "${TARGET:?TARGET envvar is required to be defined}" in
  *darwin*)
    _prefix="${APPLE_TARGET:?}-"
    fallback_llvm
    ;;
  *)
    case "$0" in
      ar | dlltool | lib | ranlib | objcopy)
        # The extra space is intentional
        _prefix='zig '
        ;;
      *)
        _prefix='llvm-'
        _suffix='-16'
        ;;
    esac
    ;;
esac

_tool="${_prefix:?}$(basename "$0")${_suffix:-}"

exec $_tool "$@"
