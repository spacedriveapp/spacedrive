#!/usr/bin/env -S bash -euo pipefail

echo "Download shaderc..."
mkdir -p shaderc/third_party/{glslang,spirv-headers,spirv-tools}

curl_tar 'https://github.com/google/shaderc/archive/refs/tags/v2023.7.tar.gz' shaderc 1

# Thrid party deps
curl_tar 'https://github.com/KhronosGroup/glslang/archive/48f9ed8b08be974f4e463ef38136c8f23513b2cf.tar.gz' shaderc/third_party/glslang 1
cp -a shaderc/third_party/glslang/LICENSE.txt shaderc/LICENSE.glslang

curl_tar 'https://github.com/KhronosGroup/SPIRV-Headers/archive/4183b260f4cccae52a89efdfcdd43c4897989f42.tar.gz' shaderc/third_party/spirv-headers 1
cp -a shaderc/third_party/spirv-headers/LICENSE shaderc/LICENSE.spirv-headers

curl_tar 'https://github.com/KhronosGroup/SPIRV-Tools/archive/360d469b9eac54d6c6e20f609f9ec35e3a5380ad.tar.gz' shaderc/third_party/spirv-tools 1
cp -a shaderc/third_party/spirv-tools/LICENSE shaderc/LICENSE.spirv-tools

sed -i '/add_subdirectory(test)/d' shaderc/third_party/spirv-tools/CMakeLists.txt
sed -i '/add_subdirectory(tools)/d' shaderc/third_party/spirv-tools/CMakeLists.txt
sed -i '/add_subdirectory(examples)/d' shaderc/third_party/spirv-tools/CMakeLists.txt

# Remove some superfluous files
rm -rf shaderc/{.github,android_test,build_overrides,examples,kokoro}
rm -rf shaderc/third_party/glslang/{.github,Test,build_overrides,gtests,kokoro,ndk_test}
rm -rf shaderc/third_party/spirv-headers/{.github,tests,tools}
rm -rf shaderc/third_party/spirv-tools/{.github,android_test,build_overrides,docs,examples,kokoro,test,tools}

# Backup source
bak_src 'shaderc'

mkdir -p shaderc/build
cd shaderc/build

echo "Build shaderc..."
cmake \
  -DSPIRV_SKIP_TESTS=On \
  -DENABLE_EXCEPTIONS=On \
  -DSHADERC_SKIP_TESTS=On \
  -DSHADERC_SKIP_EXAMPLES=On \
  -DSPIRV_SKIP_EXECUTABLES=On \
  -DSPIRV_TOOLS_BUILD_STATIC=On \
  -DSHADERC_SKIP_COPYRIGHT_CHECK=On \
  -DENABLE_PCH=Off \
  -DBUILD_TESTS=Off \
  -DENABLE_CTEST=Off \
  -DBUILD_TESTING=Off \
  -DSPIRV_CHECK_CONTEXT=Off \
  -DENABLE_GLSLANG_BINARIES=Off \
  ..

ninja -j"$(nproc)"

ninja install

echo "Libs: -lstdc++" >>"${PREFIX}/lib/pkgconfig/shaderc_static.pc"
echo "Libs: -lstdc++" >>"${PREFIX}/lib/pkgconfig/shaderc_combined.pc"

# Ensure whomever links against shaderc uses the combined version,
# which is a static library containing libshaderc and all of its dependencies.
ln -sf shaderc_combined.pc "$PREFIX"/lib/pkgconfig/shaderc.pc
