#!/usr/bin/env bash

set -euo pipefail

# Shim Microsoft's native llvm-rc help, so meson identifies us as such
if [ "$#" -eq 1 ] && [ "$1" == '/?' ]; then
  echo "Microsoft Resource Compiler"
  exit 0
fi

argv=("$@")
files=()
while [ "${#argv[@]}" -gt 0 ] && [ -f "${argv[-1]}" ] && [ "${argv[-1]: -3:3}" == '.rc' ]; do
  files+=("${argv[-1]}")
  unset 'argv[-1]'
done

exec zig rc "${argv[@]}" -- "${files[@]}"
