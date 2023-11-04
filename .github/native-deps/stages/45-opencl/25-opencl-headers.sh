#!/usr/bin/env -S bash -euo pipefail

echo "Download opencl headers..."

mkdir -p opencl-headers

curl_tar 'https://github.com/KhronosGroup/OpenCL-Headers/archive/refs/tags/v2023.04.17.tar.gz' opencl-headers 1

# Remove some superfluous files
rm -rf opencl-headers/{.github,tests}

# Backup source
bak_src 'opencl-headers'

# Install
mkdir -p "${PREFIX}/include"
mv 'opencl-headers/CL' "${PREFIX}/include/"
