#!/usr/bin/env bash

set -euo pipefail

# Ensure file exists before sourcing
touch /etc/environment
# Import any environment specific variables
set -o allexport
# shellcheck disable=SC1091
. /etc/environment
set +o allexport

# Configure cross compiler environment variables
export RC="rc"
export CC="cc"
export LD="cc"
export AR="ar"
export NM="nm"
export CXX="c++"
export STRIP="strip"
export RANLIB="ranlib"
export WINDRES="windres"
export DLLTOOL="dlltool"
export OBJCOPY="objcopy"
export OBJDUMP="objdump"
export PKG_CONFIG="pkg-config"
export PKG_CONFIG_LIBDIR="${PREFIX}/lib/pkgconfig:${PREFIX}/share/pkgconfig"

case "$TARGET" in
  x86_64*)
    export AS="nasm"
    ;;
  aarch64*)
    export AS="cc -xassembler"
    ;;
esac

FFLAGS="-fasynchronous-unwind-tables -fexceptions -fstack-protector-strong"
case "$TARGET" in
  x86_64*)
    FFLAGS="${FFLAGS} -fcf-protection"
    ;;
esac

CFLAGS="-I${PREFIX}/include -pipe -Wall -Werror=format-security"
LDFLAGS="-L${PREFIX}/lib -pipe"
case "$TARGET" in
  *linux*)
    FFLAGS="-fno-semantic-interposition"
    CFLAGS="${CFLAGS} -D_FORTIFY_SOURCE=2 -D_GLIBCXX_ASSERTIONS"
    LDFLAGS="${LDFLAGS} -Wl,-z,relro,-z,now,-z,defs"

    case "$TARGET" in
      x86_64*)
        FFLAGS="${FFLAGS} -fstack-check -fstack-clash-protection"
        ;;
      aarch64*)
        # https://github.com/ziglang/zig/issues/17430#issuecomment-1752592338
        FFLAGS="${FFLAGS} -fno-stack-protector -fno-stack-check"
        ;;
    esac
    ;;
  *darwin*)
    # Apple tools and linker fails to LTO static libraries
    # https://github.com/tpoechtrager/osxcross/issues/366
    export LTO=0

    # Ugly workaround for apple linker not finding the macOS SDK's Framework directory
    ln -fs "${MACOS_SDKROOT}/System" '/System'

    export SDKROOT="$MACOS_SDKROOT"

    case "$TARGET" in
      x86_64*)
        export CMAKE_OSX_ARCHITECTURES='x86_64'
        export MACOSX_DEPLOYMENT_TARGET="10.15"
        export CMAKE_APPLE_SILICON_PROCESSOR='x86_64'
        LDFLAGS="${LDFLAGS} -Wl,-arch,x86_64"
        ;;
      aarch64*)
        export CMAKE_OSX_ARCHITECTURES='aarch64'
        export MACOSX_DEPLOYMENT_TARGET="11.0"
        export CMAKE_APPLE_SILICON_PROCESSOR='aarch64'
        LDFLAGS="${LDFLAGS} -Wl,-arch,arm64"
        ;;
    esac

    FFLAGS="${FFLAGS} -fstack-check"

    # https://github.com/tpoechtrager/osxcross/commit/3279f86
    CFLAGS="${CFLAGS} -D__ENVIRONMENT_OS_VERSION_MIN_REQUIRED__=$(LC_ALL=C printf '%.2f' "11.0" | tr -d '.') -mmacos-version-min=${MACOSX_DEPLOYMENT_TARGET} -mmacosx-version-min=${MACOSX_DEPLOYMENT_TARGET}"
    LDFLAGS="-fuse-ld=$(command -v "${APPLE_TARGET:?}-ld") -L${SDKROOT}/usr/lib -L${SDKROOT}/usr/lib/system -F${SDKROOT}/System/Library/Frameworks ${LDFLAGS}"
    ;;
  *windows*)
    # Zig doesn't support stack probing on Windows
    # https://github.com/ziglang/zig/blob/b3462b7cec9931cd3747f10714954eb8efe00c04/src/target.zig#L326-L329
    FFLAGS="${FFLAGS} -fno-stack-check"
    CFLAGS="${CFLAGS} -D_FORTIFY_SOURCE=2 -D_GLIBCXX_ASSERTIONS"
    ;;
esac
export CFLAGS="${CFLAGS} ${FFLAGS}"
export LDFLAGS="${LDFLAGS} ${FFLAGS}"
export CXXFLAGS="${CFLAGS}"

bak_src() {
  if ! { [ "$#" -eq 1 ] && [ -d "$1" ]; }; then
    echo "bak_src: <SRC_DIR>" >&2
    exit 1
  fi

  set -- "$(CDPATH='' cd -- "$1" && pwd -P)"

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
