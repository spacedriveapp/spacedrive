#!/usr/bin/env -S bash -euo pipefail

echo "Download shaderc..."
mkdir -p shaderc/{build,third_party/{glslang,spirv-headers,spirv-tools}}

curl -LSs 'https://github.com/google/shaderc/archive/refs/tags/v2023.7.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C shaderc

# Thrid party deps
curl -LSs 'https://github.com/KhronosGroup/glslang/archive/48f9ed8b08be974f4e463ef38136c8f23513b2cf.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C shaderc/third_party/glslang
curl -LSs 'https://github.com/KhronosGroup/SPIRV-Headers/archive/4183b260f4cccae52a89efdfcdd43c4897989f42.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C shaderc/third_party/spirv-headers
curl -LSs 'https://github.com/KhronosGroup/SPIRV-Tools/archive/360d469b9eac54d6c6e20f609f9ec35e3a5380ad.tar.gz' \
  | bsdtar -xf- --strip-component 1 -C shaderc/third_party/spirv-tools

cd shaderc/build

echo "Build shaderc..."
cmake \
  -DSPIRV_SKIP_TESTS=On \
  -DSPIRV_CHECK_CONTEXT=Off \
  -DSPIRV_SKIP_EXECUTABLES=On \
  -DSPIRV_TOOLS_BUILD_STATIC=On \
  -DENABLE_PCH=Off \
  -DENABLE_CTEST=Off \
  -DENABLE_EXCEPTIONS=On \
  -DENABLE_GLSLANG_BINARIES=Off \
  -DSHADERC_SKIP_TESTS=On \
  -DSHADERC_SKIP_EXAMPLES=On \
  -DSHADERC_SKIP_COPYRIGHT_CHECK=On \
  ..

ninja -j"$(nproc)"

ninja install

# TODO: I don't think this is necessary
# for some reason, this does not get installed...
# cp libshaderc_util/libshaderc_util.a "${PREFIX}/lib"

echo "Libs: -lstdc++" >>"${PREFIX}/lib/pkgconfig/shaderc_static.pc"
echo "Libs: -lstdc++" >>"${PREFIX}/lib/pkgconfig/shaderc_combined.pc"

# Ensure whomever links against shaderc uses the combined version,
# which is a static library containing libshaderc and all of its dependencies.
cp "$PREFIX"/lib/pkgconfig/{shaderc_combined,shaderc}.pc
