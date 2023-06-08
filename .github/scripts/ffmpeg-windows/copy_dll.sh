#!/bin/bash

set -e          # exit immediate if an error occurs in a pipeline
set -E          # make commands inherit ERR trap
set -u          # don't allow not set variables to be utilized
set -o pipefail # trace ERR through pipes
set -o errtrace # trace ERR through 'time command' and other functions

if [ -z "${FFBUILD_PREFIX:-}" ]; then
  echo "Missing FFBUILD_PREFIX envvar" >&2
  exit 1
fi

find "$FFBUILD_PREFIX/bin" -name '*.dll' -print0 | while IFS= read -r -d '' _dll; do
  _dir="$(dirname "$_dll")"
  _dir="${_dir#"${FFBUILD_PREFIX}/bin/"}"
  if [ -z "$_dir" ]; then
    _dir='.'
  elif [ "$_dir" != '.' ]; then
    mkdir -p "${FFBUILD_PREFIX}/lib/${_dir}"
  fi
  mv -v "$_dll" "${FFBUILD_PREFIX}/lib/${_dir}/"
done
