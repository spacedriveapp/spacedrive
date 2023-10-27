#!/usr/bin/env -S bash -euo pipefail

echo "Download soxr..."
mkdir -p soxr/build

curl -LSs 'https://downloads.sourceforge.net/project/soxr/soxr-0.1.3-Source.tar.xz' \
  | bsdtar -xf- --strip-component 1 -C soxr

for patch in "$PREFIX"/patches/*; do
  patch -F5 -lp1 -d soxr -t < "$patch"
done

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
