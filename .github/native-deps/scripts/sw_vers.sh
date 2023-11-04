#!/usr/bin/env bash

set -euo pipefail

case "$TARGET" in
  *darwin*) ;;
  *)
    echo "Not Darwin target" >&2
    exit 1
    ;;
esac

_help="Usage: sw_vers [--help|--productName|--productVersion|--productVersionExtra|--buildVersion]"
_name="macOS"
_build="23a344"
_version="${MACOS_SDK_VERSION:?Missing macOS SDK version}}"

if [ "$#" -eq 0 ]; then
  cat <<EOF
ProductName:  ${_name}
ProductVersion:  ${_version}
BuildVersion:  ${_build}
EOF
fi

case "${*: -1}" in
  --help)
    echo "$_help"
    ;;
  --productName)
    echo "$_name"
    ;;
  --productVersion)
    echo "$_version"
    ;;
  --productVersionExtra)
    echo ""
    ;;
  --buildVersion)
    echo "$_build"
    ;;
  *)
    echo "sw_vers: unrecognized option \`${*: -1}'"
    echo "$_help"
    ;;
esac
