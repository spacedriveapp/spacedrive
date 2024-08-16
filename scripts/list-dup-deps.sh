#!/usr/bin/env bash

set -euo pipefail

_root="$(CDPATH='' cd "$(dirname "$0")" && pwd -P)"

grep -Po '^\s+"[\w-]+\s+\d(\.\d+)*[^"]*"' "${_root}/../Cargo.lock" \
  | xargs printf '%s\n' \
  | sort -u -k 1b,2V
