#!/usr/bin/env -S bash -euo pipefail

echo "Download ffmpeg..."
mkdir -p ffmpeg

curl_tar 'https://github.com/FFmpeg/FFmpeg/archive/refs/tags/n6.0.tar.gz' ffmpeg 1

# Backup source
bak_src 'ffmpeg'

cd ffmpeg

echo "Build ffmpeg..."

export CFLAGS="${CFLAGS:-} ${SHARED_FLAGS:-}"
export CXXFLAGS="${CXXFLAGS:-} ${SHARED_FLAGS:-}"

env_specific_arg=()

if [ "$(uname -m)" = "${TARGET%%-*}" ] && (case "$TARGET" in *linux* | x86_64-windows*) exit 0 ;; *) exit 1 ;; esac) then
  # zig cc doesn't support compiling cuda code yet, so we use the host clang for it
  # Unfortunatly that means we only suport cuda in the same architecture as the host system
  # https://github.com/ziglang/zig/pull/10704#issuecomment-1023616464
  env_specific_arg+=(
    --nvcc="clang-16 -target ${TARGET}"
    --enable-cuda-llvm
    --enable-ffnvcodec
    --disable-cuda-nvcc
  )
else
  # There are no Nvidia GPU drivers for macOS or Windows on ARM
  env_specific_arg+=(
    --nvcc=false
    --disable-cuda-llvm
    --disable-ffnvcodec
    --disable-cuda-nvcc
  )
fi

case "$TARGET" in
  x86_64-darwin*)
    # FIX-ME: x86 external asm is disabled on macOS, because ld64 segfaults when linking assembly for some reason
    env_specific_arg+=(
      --x86asmexe=false
      --disable-x86asm
      --disable-altivec
      --disable-vsx
      --disable-power8
      --disable-armv5te
      --disable-armv6
      --disable-armv6t2
      --disable-vfp
      --disable-neon
      --disable-mipsdsp
      --disable-mipsdspr2
      --disable-msa
      --disable-mipsfpu
      --disable-mmi
      --disable-lsx
      --disable-lasx
      --disable-rvv
    )
    ;;
  x86_64*)
    env_specific_arg+=(
      --x86asmexe=nasm
      --enable-x86asm
      --disable-altivec
      --disable-vsx
      --disable-power8
      --disable-armv5te
      --disable-armv6
      --disable-armv6t2
      --disable-vfp
      --disable-neon
      --disable-mipsdsp
      --disable-mipsdspr2
      --disable-msa
      --disable-mipsfpu
      --disable-mmi
      --disable-lsx
      --disable-lasx
      --disable-rvv
    )
    ;;
  aarch64*)
    env_specific_arg+=(
      --enable-vfp
      --enable-neon
      --x86asmexe=false
      --disable-altivec
      --disable-vsx
      --disable-power8
      --disable-amd3dnow
      --disable-amd3dnowext
      --disable-mmx
      --disable-mmxext
      --disable-sse
      --disable-sse2
      --disable-sse3
      --disable-ssse3
      --disable-sse4
      --disable-sse42
      --disable-avx
      --disable-xop
      --disable-fma3
      --disable-fma4
      --disable-avx2
      --disable-avx512
      --disable-avx512icl
      --disable-aesni
      --disable-armv5te
      --disable-armv6
      --disable-armv6t2
      --disable-x86asm
      --disable-mipsdsp
      --disable-mipsdspr2
      --disable-msa
      --disable-mipsfpu
      --disable-mmi
      --disable-lsx
      --disable-lasx
      --disable-rvv
    )
    ;;
esac

case "$TARGET" in
  *darwin*)
    env_specific_arg+=(
      # TODO: Metal suport is disabled because no open source compiler is available for it
      # TODO: Maybe try macOS own metal compiler under darling? https://github.com/darlinghq/darling/issues/326
      # TODO: Add support for vulkan (+ libplacebo) on macOS with MoltenVK
      --sysroot="${MACOS_SDKROOT:?Missing macOS SDK path}"
      --disable-metal
      --disable-vulkan
      --disable-libshaderc
      --disable-libplacebo
      --disable-mediafoundation
      --disable-amd3dnow
      --disable-amd3dnowext
      --disable-fast-unaligned
      --enable-lto
      --enable-pthreads
      --enable-coreimage
      --enable-videotoolbox
      --enable-avfoundation
      --enable-audiotoolbox
    )
    ;;
  *linux*)
    env_specific_arg+=(
      --disable-coreimage
      --disable-videotoolbox
      --disable-avfoundation
      --disable-audiotoolbox
      --disable-mediafoundation
      --enable-lto
      --enable-vulkan
      --enable-pthreads
      --enable-libshaderc
      --enable-libplacebo
    )
    ;;
  *windows*)
    # TODO: Add support for pthreads on Windows (zig doesn't ship pthreads-w32 from mingw64)
    # TODO: Add support for mediafoundation on Windows (zig doesn't seem to have the necessary bindings to it yet)
    # FIX-ME: LTO isn't working on Windows rn
    env_specific_arg+=(
      --disable-lto
      --disable-pthreads
      --disable-coreimage
      --disable-videotoolbox
      --disable-avfoundation
      --disable-audiotoolbox
      --disable-mediafoundation
      --enable-vulkan
      --enable-libshaderc
      --enable-libplacebo
    )
    ;;
esac

case "$TARGET" in
  *darwin* | aarch64-windows*) ;;
    # Apple only support its own APIs for hardware (de/en)coding on macOS
    # Windows on ARM doesn't have external GPU support yet
  *)
    env_specific_arg+=(
      --enable-amf
      --enable-libvpl
    )
    ;;
esac

_arch="${TARGET%%-*}"
case "$TARGET" in
  aarch64-darwin*)
    _arch=arm64
    ;;
esac

if ! ./configure \
  --cpu="$_arch" \
  --arch="$_arch" \
  --prefix="$OUT" \
  --target-os="$(
    case "$TARGET" in
      *linux*)
        echo "linux"
        ;;
      *darwin*)
        echo "darwin"
        ;;
      *windows*)
        echo "mingw64"
        ;;
    esac
  )" \
  --cc=cc \
  --nm=llvm-nm-16 \
  --ar=ar \
  --cxx=c++ \
  --strip=llvm-strip-16 \
  --ranlib=ranlib \
  --host-cc=clang-16 \
  --windres="windres" \
  --pkg-config=pkg-config \
  --pkg-config-flags="--static" \
  --disable-static \
  --disable-debug \
  --disable-doc \
  --disable-htmlpages \
  --disable-txtpages \
  --disable-manpages \
  --disable-podpages \
  --disable-indevs \
  --disable-outdevs \
  --disable-parser=avs2 \
  --disable-parser=avs3 \
  --disable-postproc \
  --disable-programs \
  --disable-libwebp \
  --disable-sdl2 \
  --disable-metal \
  --disable-opengl \
  --disable-network \
  --disable-openssl \
  --disable-schannel \
  --disable-securetransport \
  --disable-xlib \
  --disable-libxcb \
  --disable-libxcb-shm \
  --disable-libxcb-xfixes \
  --disable-libxcb-shape \
  --disable-libv4l2 \
  --disable-v4l2-m2m \
  --disable-w32threads \
  --disable-xmm-clobber-test \
  --disable-neon-clobber-test \
  --enable-asm \
  --enable-avcodec \
  --enable-avfilter \
  --enable-avformat \
  --enable-bzlib \
  --enable-cross-compile \
  --enable-gpl \
  --enable-inline-asm \
  --enable-libdav1d \
  --enable-libmp3lame \
  --enable-libopus \
  --enable-libsoxr \
  --enable-libsvtav1 \
  --enable-libtheora \
  --enable-libvorbis \
  --enable-libvpx \
  --enable-libx264 \
  --enable-libx265 \
  --enable-libzimg \
  --enable-lzma \
  --enable-opencl \
  --enable-optimizations \
  --enable-postproc \
  --enable-shared \
  --enable-swscale \
  --enable-version3 \
  --enable-zlib \
  "${env_specific_arg[@]}"; then
  cat ffbuild/config.log >&2
  exit 1
fi

case "$TARGET" in
  *linux*)
    # Replace incorrect identifyed sysctl as enabled on linux
    sed -i 's/#define HAVE_SYSCTL 1/#define HAVE_SYSCTL 0/' config.h
    ;;
esac

make -j"$(nproc)" V=1

make PREFIX="$OUT" install
