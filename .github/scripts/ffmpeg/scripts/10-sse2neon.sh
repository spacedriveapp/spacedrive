#!/usr/bin/env -S bash -euo pipefail

case "$TARGET" in
  aarch64*) ;;
  *)
    export UNSUPPORTED=1
    exit 1
    ;;
esac

echo "Download sse2neon..."

mkdir -p "${PREFIX}/include"

curl -LSs 'https://github.com/DLTcollab/sse2neon/archive/refs/tags/v1.6.0.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C "${PREFIX}/include" 'sse2neon-1.6.0/sse2neon.h'

curl -LSs 'https://raw.githubusercontent.com/HandBrake/HandBrake/172cd5d/contrib/sse2neon/A01-types-fix.patch' \
  | patch -F5 -lp1 -d "${PREFIX}/include" -t
