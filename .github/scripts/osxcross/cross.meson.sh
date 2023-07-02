#!/usr/bin/env bash

set -e          # exit immediate if an error occurs in a pipeline
set -E          # make commands inherit ERR trap
set -u          # don't allow not set variables to be utilized
set -o pipefail # trace ERR through pipes
set -o errtrace # trace ERR through 'time command' and other functions

declare -A cpu_arch_mappings=(
  ["x86_64"]="x86_64"
  ["armv8"]="aarch64"
)

# Get darwin version and build compiler triple
DARWIN_VERSION="$(basename "$(realpath "$(command -v "oa64-clang")")" | awk -F- '{print $3}')"

for TARGET_CPU in "${!cpu_arch_mappings[@]}"; do
  TARGET_ARCH="${cpu_arch_mappings[$TARGET_CPU]}"

  TRIPLE="${TARGET_ARCH}-apple-${DARWIN_VERSION}"

  # Check macOS clang exists
  CC="${TRIPLE}-clang"
  if ! command -v "$CC" 2>/dev/null; then
    echo "$CC not found" >&2
    exit 1
  fi

  # Get osxcross root directory
  _osxcross_root="$(dirname "$(dirname "$(command -v "$CC")")")"

  # Check SDK exists
  SYSROOT="${_osxcross_root}/SDK/MacOSX${MACOS_VERSION}.sdk"
  if ! [ -d "$SYSROOT" ]; then
    echo "Invalid MacOS version: $MACOS_VERSION" >&2
    exit 1
  fi

  # Configure Meson for osxcross
  # TODO: This should in the base osxcross image
  cat <<EOF >"${_osxcross_root}/${TRIPLE}.meson"
[binaries]
c = '$CC'
cpp = '${TRIPLE}-clang++'
ld = '$CC'
ar = '${TRIPLE}-ar'
strip = '${TRIPLE}-strip'
cmake = '${TRIPLE}-cmake'
ranlib = '${TRIPLE}-ranlib'
pkgconfig = '${TRIPLE}-pkg-config'

[properties]
sys_root = '${SYSROOT}'

[host_machine]
system = 'darwin'
cpu_family = '$TARGET_ARCH'
cpu = '$TARGET_CPU'
endian = 'little'
EOF
done
