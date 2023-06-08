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

if [ "${1:-}" = 'final' ]; then
  find "${FFBUILD_PREFIX}/lib" -name '*.dll' -print0 | while IFS= read -r -d '' _dll; do
    x86_64-w64-mingw32-strip -s "$_dll"
    cp -av "$_dll" /opt/dlls/bin

    _dir="$(dirname "$_dll")"
    _dir="${_dir#"${FFBUILD_PREFIX}/lib/"}"
    if [ -z "$_dir" ]; then
      _dir='.'
    fi

    _name="$(basename "$_dll" '.dll')"

    if [ -f "${FFBUILD_PREFIX}/lib/${_dir}/${_name}.dll.a" ]; then
      mkdir -p "/opt/dlls/lib/${_dir}"
      cp -av "${FFBUILD_PREFIX}/lib/${_dir}/${_name}.dll.a" "/opt/dlls/lib/${_dir}/"
    fi

    if [ -f "${FFBUILD_PREFIX}/lib/${_dir}/${_name}.lib" ]; then
      mkdir -p "/opt/dlls/lib/${_dir}"
      cp -av "${FFBUILD_PREFIX}/lib/${_dir}/${_name}.lib" "/opt/dlls/lib/${_dir}/"
    fi
  done
else
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

fi
