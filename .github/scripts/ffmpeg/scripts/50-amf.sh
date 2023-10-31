#!/usr/bin/env -S bash -euo pipefail

case "$TARGET" in
  *macos*)
    export UNSUPPORTED=1
    exit 1
    ;;
esac

echo "Download AMF..."

mkdir -p amf

curl_tar 'https://github.com/HandBrake/HandBrake-contribs/releases/download/contribs/AMF-1.4.30-slim.tar.gz' 'amf' 1

# Remove some superfluous files
rm -rf amf/{.github,amf/{doc,public/{make,props,proj,common,src,samples}}}

# Backup source
bak_src 'amf'

# Install
mkdir -p "${PREFIX}/include"
mv 'amf/amf/public/include' "${PREFIX}/include/AMF"
