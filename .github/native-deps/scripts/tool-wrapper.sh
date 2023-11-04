#!/usr/bin/env bash

set -euo pipefail

case "${TARGET:?TARGET envvar is required to be defined}" in
  *darwin*)
    _prefix='llvm-'
    _suffix='-16'
    ;;
  *)
    # The extra space is intentional
    _prefix='zig '
    _suffix=''
    ;;
esac

_tool="${_prefix}$(basename "$0")${_suffix}"

exec $_tool "$@"
