#!/usr/bin/env bash

set -euo pipefail

# Import any environment specific variables
set -o allexport
# shellcheck disable=SC1091
. /etc/environment
set +o allexport

# COnfigure cross compiler environment variables
export LD="zig-ld"
export CC="zig-cc"
export AR="zig ar"
export CXX="zig-c++"
export STRIP="true"
export RANLIB="zig ranlib"
export DLLTOOL="zig dlltool"
export PKG_CONFIG="pkg-config"
export PKG_CONFIG_LIBDIR="${PREFIX}/lib/pkgconfig:${PREFIX}/share/pkgconfig"

case "$TARGET" in
  *linux*)
    export CFLAGS="-I${PREFIX}/include -pipe -fPIC -DPIC -D_FORTIFY_SOURCE=2 -fstack-protector-strong -fstack-clash-protection -pthread -fvisibility=hidden -fno-semantic-interposition -flto=auto"
    export LDFLAGS="-L${PREFIX}/lib -pipe -fstack-protector-strong -fstack-clash-protection -Wl,-z,relro,-z,now -pthread -lm -flto=auto"
    export CXXFLAGS="$CFLAGS"
    ;;
  *windows*)
    export CFLAGS="-I${PREFIX}/include -pipe -D_FORTIFY_SOURCE=2 -fstack-protector-strong -flto=auto"
    export LDFLAGS="-L${PREFIX}/lib -pipe -fstack-protector-strong -flto=auto"
    export CXXFLAGS="$CFLAGS"

    case "$TARGET" in
      *-gnu)
        export WINAPI_NO_BUNDLED_LIBRARIES=1
        ;;
    esac
    ;;
esac

cd /srv

# Source stage script to compile current library
(
  _exit=0
  UNSUPPORTED=0
  trap '_exit=$?; if [ "$UNSUPPORTED" -eq 1 ]; then echo "Stage ignored in current environment" >&2; _exit=0; fi; exit $_exit' EXIT

  # Add wrappers to PATH
  export PATH="${PREFIX}/wrapper:${PATH}"

  set -x

  # shellcheck disable=SC1091
  . /srv/stage.sh
)

# Move cmake files in share to lib
if [ -d "${PREFIX}/share/cmake" ]; then
  mkdir -p "${PREFIX}/lib/cmake"
  mv "$PREFIX"/share/cmake/* "${PREFIX}/lib/cmake"
fi

# Move pkgconfig files in share to lib
if [ -d "${PREFIX}/share/pkgconfig" ]; then
  mkdir -p "${PREFIX}/lib/pkgconfig"
  mv "$PREFIX"/share/pkgconfig/* "${PREFIX}/lib/pkgconfig"
fi

# Remove superfluous files
rm -rf "${PREFIX:?}"/{bin,etc,man,lib/*.{.la,.so*,.dll.a},share}

# Copy licenses
while IFS= read -r _license; do
  case "${_license}" in
    # Ignore license for tests, examples, contrib, ..., as we are not compiling, running or distributing those
    *.sh | *.cfg | *.build | */test/* | */tests/* | */demos/* | */build/* | \
      */utils/* | */contrib/* | */examples/* | */3rdparty/* | */third_party/*)
      continue
      ;;
  esac

  mkdir -p "${PREFIX}/licenses/"

  # Rename license files to include the package name
  cp "$_license" "${PREFIX}/licenses/$(dirname "${_license#/srv/}" | tr '/' '-').$(basename "$_license" .txt)"
done < <(find /srv -type f \( -iname 'license*' -o -iname 'copying*' \) -not -wholename "${PREFIX}/**")
