#!/usr/bin/env sh

set -eu

version() {
  echo "$@" | awk -F. '{ printf("%d%03d%03d%03d\n", $1,$2,$3,$4); }'
}

APPRUN_ENV_ORIGINAL_WORKDIR=$(pwd -P)

# Change workdir to $APPDIR. Wrapper script is located in $APPDIR/usr/bin
CDPATH="" cd "$(dirname "$0")/../.."

# AppImage root path
ORIGIN="$(pwd -P)"

if ! { [ -f 'AppRun.env' ] && [ -r 'AppRun.env' ]; }; then
  echo 'AppRun.env not found. Invalid AppImage environment.' >&2
  exit 1
fi

# Import AppRun environment variables
set -o allexport
# shellcheck disable=SC1091
. AppRun.env
set +o allexport

# Change EXEC_PATH to our wrapped binary
APPDIR_EXEC_PATH="$0.orig"
if ! { [ -x "$APPDIR_EXEC_PATH" ] && [ -f "$APPDIR_EXEC_PATH" ]; }; then
  echo "Missing wrapped binary" >&2
  exit 1
fi

# Handle glibc version
APPDIR_GLIBC_VERSION="$(awk -F'=' '$1 == "APPDIR_LIBC_VERSION" { print $2 }' AppRun.env)"
SYSTEM_GLIBC_VERSION="$(ldd --version | head -n1 | grep -i -e 'glibc' -e 'gnu' | tr -s ' ' | tr '[:space:]' '\n' | grep '^[0-9]\+\(\.[0-9]\+\)\+$')"
if [ -z "$SYSTEM_GLIBC_VERSION" ]; then
  echo "WARNING: Couldn't detect Glibc version, assuming same version as AppImage" >&2
  echo "Please report this issue (with the following message) at: https://github.com/spacedriveapp/spacedrive/issues/new/choose" >&2
  ldd --version || true
  SYSTEM_GLIBC_VERSION="$APPDIR_GLIBC_VERSION"
fi

# Change dirs to correct runtime according to system's glibc version
if [ "$(version "$SYSTEM_GLIBC_VERSION")" -lt "$(version "$APPDIR_GLIBC_VERSION")" ]; then
  cd runtime/compat
  LD_LIBRARY_PATH="${APPDIR_LIBRARY_PATH_ENV:?}:${APPDIR_LIBC_LIBRARY_PATH_ENV:?}:${APPRUN_ENV_ORIG_PREFIX:?}:${LD_LIBRARY_PATH:-}"
else
  cd runtime/default
  LD_LIBRARY_PATH="${APPDIR_LIBRARY_PATH_ENV:?}:${APPRUN_ENV_ORIG_PREFIX:?}:${LD_LIBRARY_PATH:-}"
fi

APPRUN_ENV_RUNTIME="$(pwd -P)"

export ORIGIN
export LD_LIBRARY_PATH
export APPDIR_EXEC_PATH
export APPRUN_ENV_RUNTIME
export APPRUN_ENV_ORIGINAL_WORKDIR

exec "$APPDIR_EXEC_PATH" "$@"
