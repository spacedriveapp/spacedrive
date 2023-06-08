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

get_dir() {
  local _dir

  if [ "$#" -ne 2 ]; then
    echo 'get_dir: <path> <base_dir>'
  fi

  _dir="$(dirname "$1")"
  _dir="${_dir#"${2}/"}"
  if [ -z "$_dir" ]; then
    _dir='.'
  fi

  echo "$_dir"
}

if [ "${1:-}" = 'final' ]; then
  find "$FFBUILD_PREFIX" -name '*.dll' -print0 | while IFS= read -r -d '' _dll; do
    x86_64-w64-mingw32-strip -s "$_dll"

    _dir="$(get_dir "$_dll" "${FFBUILD_PREFIX}/")"

    mkdir -p "/opt/dlls/${_dir}"

    cp -av "$_dll" "/opt/dlls/${_dir}/"
    if [ -f "${_dll}.a" ]; then
      cp -av "${_dll}.a" "/opt/dlls/${_dir}/"
    fi

    (
      _name="$(basename "$_dll" '.dll')"
      _name="${_name#lib}"
      _name="${_name%%-*}"
      _name="${_name%%_*}"

      find "${FFBUILD_PREFIX}" -name "*${_name}*.lib" | while IFS= read -r -d '' _lib; do
        _dir="$(get_dir "$_lib" "${FFBUILD_PREFIX}/")"
        mkdir -p "/opt/dlls/${_dir}"
        cp -av "$_lib" "/opt/dlls/${_dir}/"
      done
    )
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
