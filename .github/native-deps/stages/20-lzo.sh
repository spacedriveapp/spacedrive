#!/usr/bin/env -S bash -euo pipefail

echo "Download lzo..."
mkdir -p lzo

curl_tar 'https://www.oberhumer.com/opensource/lzo/download/lzo-2.10.tar.gz' lzo 1

sed -i "/^if(0)/d" lzo/CMakeLists.txt
sed -i "/^add_test/d" lzo/CMakeLists.txt
sed -i "/^include(CTest)/d" lzo/CMakeLists.txt
sed -ie 's/^if(1)/if(0)/' lzo/CMakeLists.txt
sed -ie 's/^# main test driver/if(0)/' lzo/CMakeLists.txt

# Remove unused components
rm -r lzo/{B,util,tests,minilzo,lzotest,examples,autoconf,lzo2.pc.in,Makefile.am,Makefile.in,aclocal.m4,configure,config.hin}

# Backup source
bak_src 'lzo'

mkdir -p lzo/build
cd lzo/build

echo "Build lzo..."
cmake ..

ninja -j"$(nproc)"

ninja install
