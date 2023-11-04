#!/usr/bin/env -S bash -euo pipefail

echo "Download lcms..."
mkdir -p lcms

curl_tar 'https://github.com/mm2/Little-CMS/releases/download/lcms2.15/lcms2-2.15.tar.gz' lcms 1

case "$TARGET" in
  aarch64*)
    # Patch to enable SSE codepath on aarch64
    patch -F5 -lp1 -d lcms -t <"$PREFIX"/patches/sse2neon.patch
    ;;
esac

# Some required patches for fixing meson and windows cross-compile issues
for patch in \
  https://github.com/mm2/Little-CMS/commit/4e55c55802e4aee5f65be120291f5f4785483d98.patch \
  https://github.com/mm2/Little-CMS/commit/8ddc2681c06948eb20909cea70c1bffa10393d47.patch \
  https://github.com/mm2/Little-CMS/commit/8769c0e85b0e57de3f55936344766873fa982350.patch \
  https://github.com/mm2/Little-CMS/commit/7984408c8fe800a27175e4a8bd6115663c553ec1.patch \
  https://github.com/mm2/Little-CMS/commit/b35e2718688508dfe2591808cfc74a77490849f6.patch; do
  curl -LSs "$patch" | patch -F5 -lp1 -d lcms -t
done

sed -i "/subdir('utils')/d" lcms/meson.build
sed -i "/subdir('testbed')/d" lcms/meson.build

# Remove some superfluous files
rm -rf lcms/{.github,configure.ac,install-sh,depcomp,Makefile.in,config.sub,aclocal.m4,config.guess,ltmain.sh,m4,utils,configure,Projects,doc,testbed}

# Backup source
bak_src 'lcms'

mkdir -p lcms/build
cd lcms/build

echo "Build lcms..."
meson \
  --errorlogs \
  -Dutils=false \
  -Dsamples=false \
  -Dthreaded="$(
    case "$TARGET" in
      *windows*)
        # TODO: Add support for pthreads on Windows
        echo "false"
        ;;
      *)
        echo "true"
        ;;
    esac
  )" \
  -Dfastfloat=true \
  ..

ninja -j"$(nproc)"

ninja install
