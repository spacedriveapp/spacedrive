#!/usr/bin/env -S bash -euo pipefail

echo "Download soxr..."
mkdir -p soxr

# Original link: https://downloads.sourceforge.net/project/soxr/soxr-0.1.3-Source.tar.xz
# But sourcefourge is very bad to download from, so we use the debian source instead
curl_tar 'https://deb.debian.org/debian/pool/main/libs/libsoxr/libsoxr_0.1.3.orig.tar.xz' soxr 1

for patch in "$PREFIX"/patches/*; do
  patch -F5 -lp1 -d soxr -t < "$patch"
done

# Remove some superfluous files
rm -rf soxr/{examples,lsr-tests,msvc,tests}

# Backup source
bak_src 'soxr'

mkdir -p soxr/build
cd soxr/build

echo "Build soxr..."
cmake \
  -DBUILD_TESTS=Off \
  -DINSTALL_DOCS=Off \
  -DBUILD_EXAMPLES=Off \
  -WITH_LSR_BINDINGS=Off \
  ..

ninja -j"$(nproc)"

ninja install
