#!/usr/bin/env bash

set -euo pipefail

if ! { [ "$#" -gt 1 ] && [ -n "$1" ] && [ -n "$2" ]; }; then
  echo "Usage: $0 <URL> <DIR NAME> <STRIP>" >&2
  exit 1
fi

if [ -e "$2" ] && ! [ -d "$2" ]; then
  echo "<DIR NAME> must be a valid directory path" >&2
  exit 1
fi

case "${3:-}" in
  '')
    set -- "$1" "$2" 0
    ;;
  *[0-9]*) ;;
  *)
    echo "<STRIP> must be a valid number" >&2
    exit 1
    ;;
esac

mkdir -p "$2"

if [ -n "${4:-}" ]; then
  set -- "$1" "$2" "$3" "'$4'"
fi

_url="$1"
_cache="/root/.cache/_curl_tar/$(md5sum - <<<"$_url" | awk '{ print $1 }')"
_ciphersuites="TLS_AES_128_GCM_SHA256:TLS_CHACHA20_POLY1305_SHA256:TLS_AES_256_GCM_SHA384:ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-CHACHA20-POLY1305:ECDHE-RSA-CHACHA20-POLY1305:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384"

mkdir -p "$(dirname "$_cache")"

if ! [ -s "$_cache" ]; then
  curl --proto '=https' --tlsv1.2 --ciphers "$_ciphersuites" --silent --show-error --fail --location "$_url" >"$_cache"
fi

echo "'$3'" -C "'$2'" "${4:-}" | xargs \
  bsdtar -xf "$_cache" --no-acls --no-xattrs --no-same-owner --no-mac-metadata --no-same-permissions --strip-component
