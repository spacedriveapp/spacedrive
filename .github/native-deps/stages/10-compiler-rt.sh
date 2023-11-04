#!/usr/bin/env -S bash -euo pipefail

case "$TARGET" in
  *darwin*) ;;
  *)
    export UNSUPPORTED=1
    exit 1
    ;;
esac

# LLVM install path
LLVM_PATH="/usr/lib/llvm-16"

# Remove wrapper from PATH, because we need to call the original cmake
PATH="$(echo "${PATH}" | awk -v RS=: -v ORS=: '/\/wrapper^/ {next} {print}')"
export PATH

echo "Download llvm compiler_rt..."

mkdir -p "${LLVM_PATH}/compiler_rt/build"

curl_tar 'https://github.com/llvm/llvm-project/releases/download/llvmorg-16.0.6/cmake-16.0.6.src.tar.xz' \
  "${LLVM_PATH}/cmake" 1
curl_tar 'https://github.com/llvm/llvm-project/releases/download/llvmorg-16.0.6/compiler-rt-16.0.6.src.tar.xz' \
  "${LLVM_PATH}/compiler_rt" 1

# Link cmake files to where compiler_rt expect to find them
ln -s . "${LLVM_PATH}/cmake/modules"

cd "${LLVM_PATH}/compiler_rt/build"

_arch="${TARGET%%-*}"

# Install
cmake \
  -GNinja \
  -Wno-dev \
  -DLLVM_PATH="$LLVM_PATH" \
  -DLLVM_CMAKE_DIR="${LLVM_PATH}/cmake" \
  -DDARWIN_osx_ARCHS="$(if [ "$_arch" == 'aarch64' ]; then echo 'arm64'; else echo "$_arch"; fi)" \
  -DLLVM_MAIN_SRC_DIR="$LLVM_PATH" \
  -DCMAKE_INSTALL_PREFIX="${LLVM_PATH}/lib/clang/16" \
  -DCMAKE_TOOLCHAIN_FILE='/srv/toolchain.cmake' \
  -DDARWIN_macosx_SYSROOT="${MACOS_SDKROOT:?Missing macOS SDK path}" \
  -DDARWIN_osx_BUILTIN_ARCHS="$(if [ "$_arch" == 'aarch64' ]; then echo 'arm64'; else echo "$_arch"; fi)" \
  -DCOMPILER_RT_ENABLE_IOS=Off \
  -DCOMPILER_RT_BUILD_XRAY=Off \
  -DCOMPILER_RT_BUILD_SANITIZERS=Off \
  -DDARWIN_macosx_OVERRIDE_SDK_VERSION="${MACOS_SDK_VERSION:?Missing macOS SDK version}" \
  -DCMAKE_INTERPROCEDURAL_OPTIMIZATION=Off \
  ..

ninja -j"$(nproc)"

ninja install

# Symlink clang_rt to arch specific names
while IFS= read -r _lib; do
  _lib_name="$(basename "${_lib}" .a)"
  ln -s "${_lib_name}.a" "$(dirname "${_lib}")/${_lib_name}-${_arch}.a"
  if [ "$_arch" == 'aarch64' ]; then
    ln -s "${_lib_name}.a" "$(dirname "${_lib}")/${_lib_name}-arm64.a"
  fi
done < <(find "${LLVM_PATH}/lib/clang/16/lib/darwin/" -name 'libclang_rt.*')
