#!/usr/bin/env bash

set -xeuo pipefail

# Work-around meson not recognising able to find llvm-rc
if [ "$1" = '/?' ]; then
  echo 'LLVM Resource Converter'
fi

case "${TARGET:?TARGET envvar is required to be defined}" in
  *windows-gnu)
    set -- /I "${SYSROOT:?SYSROOT envvar is required to be defined}/lib/libc/include/any-windows-any" "$@"
    ;;
esac

exec /usr/bin/llvm-rc-16 "$@"
