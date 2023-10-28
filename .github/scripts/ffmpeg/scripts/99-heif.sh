#!/usr/bin/env -S bash -euo pipefail

echo "Download libheif..."
mkdir -p libheif/build

curl -LSs 'https://github.com/strukturag/libheif/releases/download/v1.17.1/libheif-1.17.1.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C libheif

cd libheif/build

echo "Build libheif..."
SHARED=On PREFIX=/opt/out cmake \
  -DWITH_X265=On \
  -DWITH_DAV1D=On \
  -DWITH_SvtEnc=On \
  -DWITH_LIBDE265=On \
  -DWITH_LIBSHARPYUV=On \
  -DWITH_UNCOMPRESSED_CODEC=On \
  -DWITH_REDUCED_VISIBILITY=On \
  -DENABLE_MULTITHREADING_SUPPORT=On \
  -DWITH_DEFLATE_HEADER_COMPRESSIOn=On \
  -DWITH_RAV1E=Off \
  -DWITH_KVAZAAR=Off \
  -DWITH_FUZZERS=Off \
  -DWITH_EXAMPLES=Off \
  -DBUILD_TESTING=Off \
  -DWITH_AOM_DECODER=Off \
  -DWITH_AOM_ENCODER=Off \
  -DWITH_JPEG_DECODER=Off \
  -DWITH_JPEG_ENCODER=Off \
  -DWITH_FFMPEG_DECODER=Off \
  -DWITH_OpenJPEG_DECODER=Off \
  -DWITH_OpenJPEG_ENCODER=Off \
  -DENABLE_PLUGIN_LOADING=Off \
  -DWITH_UNCOMPRESSED_CODEC=Off \
  ..

ninja -j"$(nproc)"

ninja install
