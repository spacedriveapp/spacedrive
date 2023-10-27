#!/usr/bin/env -S bash -euo pipefail

echo "Download AMF..."

mkdir -p "${PREFIX}/include/AMF"

curl -LSs 'https://github.com/HandBrake/HandBrake-contribs/releases/download/contribs/AMF-1.4.30-slim.tar.gz' \
  | bsdtar -xf- --strip-component 4 -C "${PREFIX}/include/AMF" 'AMF-1.4.30/amf/public/include'
