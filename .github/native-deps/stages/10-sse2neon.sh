#!/usr/bin/env -S bash -euo pipefail

case "$TARGET" in
  aarch64*) ;;
  *)
    export UNSUPPORTED=1
    exit 1
    ;;
esac

echo "Download sse2neon..."

mkdir -p sse2neon

curl_tar 'https://github.com/DLTcollab/sse2neon/archive/refs/tags/v1.6.0.tar.gz' 'sse2neon' 1

curl -LSs 'https://raw.githubusercontent.com/HandBrake/HandBrake/172cd5d/contrib/sse2neon/A01-types-fix.patch' \
  | patch -F5 -lp1 -d "sse2neon" -t

# Remove unused components
rm -r sse2neon/{.ci,.github,tests}

# Backup source
bak_src 'sse2neon'

# Install
mkdir -p "${PREFIX}/include"
mv sse2neon/sse2neon.h "${PREFIX}/include/"
