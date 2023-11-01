#!/usr/bin/env bash

set -euo pipefail

# Import any environment specific variables
set -o allexport
# shellcheck disable=SC1091
. /etc/environment
set +o allexport

# COnfigure cross compiler environment variables
export CC="zig-cc"
export AR="zig ar"
export RC="rc"
export NM="llvm-nm-17"
export CXX="zig-c++"
export STRIP="llvm-strip-17"
export RANLIB="zig ranlib"
export WINDRES="windres"
export DLLTOOL="zig dlltool"
export OBJCOPY="zig objcopy"
export OBJDUMP="llvm-objdump-17"
export PKG_CONFIG="pkg-config"
export PKG_CONFIG_LIBDIR="${PREFIX}/lib/pkgconfig:${PREFIX}/share/pkgconfig"

case "$TARGET" in
  *linux*)
    export CFLAGS="-I${PREFIX}/include -pipe -D_FORTIFY_SOURCE=1"
    export LDFLAGS="-L${PREFIX}/lib -pipe -Wl,-z,relro,-z,now"

    case "$TARGET" in
      x86_64*)
        export CFLAGS="${CFLAGS} -fstack-protector-strong -fstack-clash-protection"
        export LDFLAGS="${LDFLAGS} -fstack-protector-strong -fstack-clash-protection"
        ;;
      aarch64*)
        # https://github.com/ziglang/zig/issues/17430#issuecomment-1752592338
        export CFLAGS="${CFLAGS} -fno-stack-protector -fno-stack-check"
        export LDFLAGS="${LDFLAGS} -fno-stack-protector -fno-stack-check"
    esac

    export CXXFLAGS="$CFLAGS"
    export SHARED_FLAGS="${CFLAGS:-} -fno-semantic-interposition"
    ;;
  *windows*)
    export CFLAGS="-I${PREFIX}/include -pipe -D_FORTIFY_SOURCE=2 -fstack-protector-strong"
    export LDFLAGS="-L${PREFIX}/lib -pipe -fstack-protector-strong"
    export CXXFLAGS="$CFLAGS"
    ;;
esac

bak_src() {
  if ! { [ "$#" -eq 1 ] && [ -d "$1" ]; }; then
    echo "bak_src: <SRC_DIR>" >&2
    exit 1
  fi

  set -- "$(CDPATH='' cd "$1" && pwd)"

  case "$1" in
    /srv/*) ;;
    *)
      echo "Soruce dir must be under /srv" >&2
      exit 1
      ;;
  esac

  mkdir -p "${PREFIX}/srv"
  cp -at "${PREFIX}/srv" "$1"
}

cd /srv

# Source stage script to compile current library
(
  _exit=0
  UNSUPPORTED=0
  trap '_exit=$?; if [ "$UNSUPPORTED" -eq 1 ]; then echo "Stage ignored in current environment" >&2; _exit=0; fi; exit $_exit' EXIT

  # Add wrappers to PATH
  export PATH="${SYSROOT}/wrapper:${PATH}"

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
