#!/usr/bin/env -S bash -euo pipefail

echo "Download x265..."
mkdir -p x265

# Need to use master, because the latest release doesn't support optmized aarch64 and it is from 2021
curl -LSs 'https://bitbucket.org/multicoreware/x265_git/get/8ee01d45b05cdbc9da89b884815257807a514bc8.tar.bz2' \
  | bsdtar -xf- --strip-component 1 -C x265

cd x265

# Force cmake to use x265Version.txt instead of querying git or hg
sed -i '/set(X265_TAG_DISTANCE/a set(GIT_ARCHETYPE "1")' source/cmake/Version.cmake

echo "Build x265..."

common_config=(
  -DENABLE_PIC=On
  -DENABLE_CLI=Off
  -DENABLE_TESTS=Off
  -DENABLE_SHARED=Off
  -DENABLE_ASSEMBLY=On
  -DENABLE_SVT_HEVC=Off
  -DCMAKE_ASM_NASM_FLAGS=-w-macro-params-legacy
)

mkdir 8bit 10bit 12bit

cmake -S source -B 12bit \
  "${common_config[@]}" \
  -DMAIN12=On \
  -DEXPORT_C_API=Off \
  -DHIGH_BIT_DEPTH=On

ninja -C 12bit -j"$(nproc)"

cmake -S source -B 10bit \
  "${common_config[@]}" \
  -DEXPORT_C_API=Off \
  -DHIGH_BIT_DEPTH=On

ninja -C 10bit -j"$(nproc)"

cmake -S source -B 8bit \
  "${common_config[@]}" \
  -DEXTRA_LIB='x265_main10.a;x265_main12.a' \
  -DLINKED_10BIT=On \
  -DLINKED_12BIT=On \
  -DEXTRA_LINK_FLAGS=-L. \
  -DENABLE_HDR10_PLUS=On

ninja -C 8bit -j"$(nproc)"

cd 8bit

# Combine all three into libx265.a
ln -s ../12bit/libx265.a libx265_main12.a
ln -s ../10bit/libx265.a libx265_main10.a
mv libx265.a libx265_main.a

zig ar -M <<EOF
CREATE libx265.a
ADDLIB libx265_main.a
ADDLIB libx265_main10.a
ADDLIB libx265_main12.a
SAVE
END
EOF

ninja install

sed -ri 's/^(Libs.private:.*)$/\1 -lstdc++/' "${PREFIX}/lib/pkgconfig/x265.pc"
