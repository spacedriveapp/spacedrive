# escape=`

ARG OUT="/opt/out"
ARG TARGET="x86_64-linux-gnu"
ARG ZIG_VERSION="0.11.0"
ARG MESON_VERSION="1.2.3"
ARG CMAKE_VERSION="3.27.0"
ARG PATCHELF_VERSION="0.18.0"
ARG MACOS_SDK_VERSION="14.0"

#--

FROM debian:bookworm as build-base

SHELL ["bash", "-euxo", "pipefail", "-c"]

# Configure apt to be docker friendly
ADD https://gist.githubusercontent.com/HeavenVolkoff/ff7b77b9087f956b8df944772e93c071/raw `
	/etc/apt/apt.conf.d/99docker-apt-config

RUN rm -f /etc/apt/apt.conf.d/docker-clean

RUN echo 'debconf debconf/frontend select Noninteractive' | debconf-set-selections

RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt `
	apt-get update && apt-get upgrade && apt-get install -y ca-certificates

# Add LLVM 16 repository
ADD https://apt.llvm.org/llvm-snapshot.gpg.key /etc/apt/trusted.gpg.d/apt.llvm.org.asc

RUN chmod 644 /etc/apt/trusted.gpg.d/apt.llvm.org.asc

RUN echo "deb https://apt.llvm.org/bookworm/ llvm-toolchain-bookworm-16 main" `
	> /etc/apt/sources.list.d/llvm.list

# Install build dependencies
RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt `
	apt-get update && apt-get install `
	nasm `
	curl `
	make `
	patch `
	libtool `
	python3 `
	gettext `
	llvm-16 `
	autoconf `
	clang-16 `
	libc6-dev `
	pkg-config `
	ninja-build `
	libc++-16-dev `
	libc-devtools- `
	libc++abi-16-dev `
	libarchive-tools

# Configure sysroot and prefix
ARG OUT
ENV OUT="${OUT:?}"
ENV PREFIX="/opt/prefix"
ENV SYSROOT="/opt/sysroot"
ENV CCTOOLS="/opt/cctools"

# Ensure sysroot and cctools are present on PATH
ENV PATH="${CCTOOLS}/bin:${SYSROOT}/bin:$PATH"

# Create required directories
RUN mkdir -p "$OUT" "$CCTOOLS" "${PREFIX}/bin" "${SYSROOT}/bin" "${SYSROOT}/wrapper"

# Utility to download, extract and cache archived files
COPY --chmod=0750 ./scripts/curl_tar.sh "${SYSROOT}/bin/curl_tar"

# Download and install zig toolchain
ARG ZIG_VERSION
RUN --mount=type=cache,target=/root/.cache `
	curl_tar "https://ziglang.org/builds/zig-linux-$(uname -m)-${ZIG_VERSION:?}.tar.xz" "$SYSROOT" 1 `
	&& mv "${SYSROOT}/zig" "${SYSROOT}/bin/zig"

# Download and install cmake
ARG CMAKE_VERSION
RUN --mount=type=cache,target=/root/.cache `
	curl_tar "https://github.com/Kitware/CMake/releases/download/v${CMAKE_VERSION:?}/cmake-${CMAKE_VERSION:?}-linux-$(uname -m).tar.gz" "$SYSROOT" 1

# Download and install meson, with a patch to add zig support
ARG MESON_VERSION
RUN --mount=type=cache,target=/root/.cache `
	curl_tar "https://github.com/mesonbuild/meson/archive/refs/tags/${MESON_VERSION:?}.tar.gz" /srv/meson 1
ADD 'https://github.com/mesonbuild/meson/pull/12293.patch' /srv/meson/12293.patch
RUN cd /srv/meson `
	&& patch -F5 -lp1 -d /srv/meson -t < 12293.patch `
	&& packaging/create_zipapp.py --outfile "${SYSROOT}/bin/meson" --compress `
	&& rm -rf /srv/meson

# Download and install patchelf, used to do some light rpath ELF manipulation
ARG PATCHELF_VERSION
RUN --mount=type=cache,target=/root/.cache `
	curl_tar "https://github.com/NixOS/patchelf/releases/download/${PATCHELF_VERSION:?}/patchelf-${PATCHELF_VERSION:?}-$(uname -m).tar.gz" "$SYSROOT" 1

# Download and install gas-preprocessor, used by our zig wrapper to handle GNU flavored assembly files
ADD --chmod=0750 'https://github.com/libav/gas-preprocessor/raw/master/gas-preprocessor.pl' "${SYSROOT}/bin/gas-preprocessor.pl"

# Download compat tool for converting windres args to rc (required by x264 and ffmpeg)
ADD --chmod=0750 'https://raw.githubusercontent.com/FFmpeg/FFmpeg/master/compat/windows/mswindres' "${SYSROOT}/bin/windres"
RUN sed -i 's/rc.exe/rc/' "${SYSROOT}/bin/windres"

# Download compat tool for generating def files for windows
ADD --chmod=0750 'https://github.com/FFmpeg/FFmpeg/raw/master/compat/windows/makedef' "${SYSROOT}/bin/makedef"

# Workaround for -lsynchronization linking issue on Windows GNU targets
# https://github.com/ziglang/zig/issues/14919
RUN cd "${SYSROOT}/lib/libc/mingw/lib-common" `
	&& { [ -f 'synchronization.def' ] || ln -s 'api-ms-win-core-synch-l1-2-0.def' 'synchronization.def'; }

# Polyfill macOS sw_vers command
COPY --chmod=0750 ./scripts/sw_vers.sh "${SYSROOT}/bin/sw_vers"

# Create a llvm rc wrapper script with some pre-configurations
# Do not name this llvm-rc or rc.exe, to avoid cmake weird special behavior for those tool names
COPY --chmod=0750 ./scripts/rc.sh "${SYSROOT}/bin/rc"

# Script wrapper for some common build tools, auto choose between llvm, zig or apple specific tools
COPY --chmod=0750 ./scripts/tool-wrapper.sh "${SYSROOT}/bin/tool-wrapper.sh"
RUN ln -s "$(command -v tool-wrapper.sh)" "${SYSROOT}/bin/ar" `
	&& ln -s "$(command -v tool-wrapper.sh)" "${SYSROOT}/bin/nm" `
	&& ln -s "$(command -v tool-wrapper.sh)" "${SYSROOT}/bin/lib" `
	&& ln -s "$(command -v tool-wrapper.sh)" "${SYSROOT}/bin/lipo" `
	&& ln -s "$(command -v tool-wrapper.sh)" "${SYSROOT}/bin/size" `
	&& ln -s "$(command -v tool-wrapper.sh)" "${SYSROOT}/bin/otool" `
	&& ln -s "$(command -v tool-wrapper.sh)" "${SYSROOT}/bin/strip" `
	&& ln -s "$(command -v tool-wrapper.sh)" "${SYSROOT}/bin/ranlib" `
	&& ln -s "$(command -v tool-wrapper.sh)" "${SYSROOT}/bin/readelf" `
	&& ln -s "$(command -v tool-wrapper.sh)" "${SYSROOT}/bin/libtool" `
	&& ln -s "$(command -v tool-wrapper.sh)" "${SYSROOT}/bin/objdump" `
	&& ln -s "$(command -v tool-wrapper.sh)" "${SYSROOT}/bin/dlltool" `
	&& ln -s "$(command -v tool-wrapper.sh)" "${SYSROOT}/bin/objcopy" `
	&& ln -s "$(command -v tool-wrapper.sh)" "${SYSROOT}/bin/strings" `
	&& ln -s "$(command -v tool-wrapper.sh)" "${SYSROOT}/bin/bitcode-strip" `
	&& ln -s "$(command -v tool-wrapper.sh)" "${SYSROOT}/bin/install_name_tool"

#--

FROM build-base AS base-layer

# Configure macOS SDK for darwin targets
ARG MACOS_SDK_VERSION
ENV MACOS_SDK_VERSION="${MACOS_SDK_VERSION:?}"
ENV MACOS_SDKROOT="/opt/MacOSX${MACOS_SDK_VERSION}.sdk"

# Export which target we are building for
ARG TARGET
ENV TARGET="${TARGET:?}"

# Cache bust
RUN echo "Building: ${TARGET}$(case "$TARGET" in *darwin*) echo " (macOS SDK: ${MACOS_SDK_VERSION})" ;; esac)"

#--

FROM base-layer AS layer-00

# Apple specific SDK and tools
RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/00-apple/00-sdk.sh,target=/srv/00-sdk.sh `
	/srv/00-sdk.sh

RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/00-apple/01-xar.sh,target=/srv/01-xar.sh `
	/srv/01-xar.sh

RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/00-apple/02-libtapi.sh,target=/srv/02-libtapi.sh `
	/srv/02-libtapi.sh

RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/00-apple/03-cctools.sh,target=/srv/03-cctools.sh `
	/srv/03-cctools.sh

# Ensure no one tries to call the native system linker
RUN ln -s '/usr/bin/false' "${SYSROOT}/bin/ld"

# Add wrapper script for zig compilers, we need to ensure that they are called with the correct arguments
COPY --chmod=0750 ./scripts/cc.sh "${SYSROOT}/bin/cc"
RUN ln -s 'cc' "${SYSROOT}/bin/c++"
RUN chmod +x "${SYSROOT}/bin/cc" "${SYSROOT}/bin/c++"

# Create cmake and meson toolchain files
RUN --mount=type=bind,rw,source=scripts/toolchain.sh,target=/srv/toolchain.sh /srv/toolchain.sh

# Create a cmake wrapper script with some pre-configurations
COPY --chmod=0750 ./scripts/cmake.sh "${SYSROOT}/wrapper/cmake"

# Create a meson wrapper script with some pre-configurations
COPY --chmod=0750 ./scripts/meson.sh "${SYSROOT}/wrapper/meson"

# Wrapper script that configure autotools and build flags
COPY --chmod=0750 ./scripts/build.sh /srv/build.sh

#--

FROM layer-00 AS layer-10-sse2neon

RUN --mount=type=cache,target=/root/.cache --mount=type=bind,source=stages/10-sse2neon.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-00 AS layer-10-compiler-rt

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/10-compiler-rt.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-00 AS layer-10

COPY --from=layer-10-sse2neon "${PREFIX}/." "$PREFIX"
COPY --from=layer-10-compiler-rt "/usr/lib/llvm-16/lib/clang/16/." '/usr/lib/llvm-16/lib/clang/16'

#--

FROM layer-10 AS layer-20-brotli

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/20-brotli.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-10 AS layer-20-bzip2

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/20-bzip2.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-10 AS layer-20-lzo

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/20-lzo.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-10 AS layer-20-zlib

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/20-zlib.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-10 AS layer-20

COPY --from=layer-20-brotli "${PREFIX}/." "$PREFIX"
COPY --from=layer-20-bzip2 "${PREFIX}/." "$PREFIX"
COPY --from=layer-20-lzo "${PREFIX}/." "$PREFIX"
COPY --from=layer-20-zlib "${PREFIX}/." "$PREFIX"

#--

FROM layer-20 AS layer-25-lcms

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/25-lcms.sh,target=/srv/stage.sh `
	--mount=type=bind,source=patches/25-lcms,target="${PREFIX}/patches" `
	/srv/build.sh

FROM layer-20 AS layer-25-lzma

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/25-lzma.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-20 AS layer-25-ogg

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/25-ogg.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-20 AS layer-25

COPY --from=layer-25-lcms "${PREFIX}/." "$PREFIX"
COPY --from=layer-25-lzma "${PREFIX}/." "$PREFIX"
COPY --from=layer-25-ogg "${PREFIX}/." "$PREFIX"

#--

FROM layer-25 AS layer-45-dav1d

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/45-dav1d.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-25 AS layer-45-de265

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/45-de265.sh,target=/srv/stage.sh `
	--mount=type=bind,source=patches/45-de265,target="${PREFIX}/patches" `
	/srv/build.sh

FROM layer-25 AS layer-45-opencl

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/45-opencl/25-opencl-headers.sh,target=/srv/stage.sh `
	/srv/build.sh

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/45-opencl/45-opencl.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-25 AS layer-45-sharpyuv

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/45-sharpyuv.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-25 AS layer-45-vorbis

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/45-vorbis.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-25 AS layer-45

COPY --from=layer-45-dav1d "${PREFIX}/." "$PREFIX"
COPY --from=layer-45-de265 "${PREFIX}/." "$PREFIX"
COPY --from=layer-45-opencl "${PREFIX}/." "$PREFIX"
COPY --from=layer-45-sharpyuv "${PREFIX}/." "$PREFIX"
COPY --from=layer-45-vorbis "${PREFIX}/." "$PREFIX"

#--

FROM layer-45 AS layer-50-amf

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/50-amf.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50-nvenc

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/50-nvenc.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50-lame

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/50-lame.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50-onevpl

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/50-onevpl.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50-opus

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/50-opus.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50-soxr

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/50-soxr.sh,target=/srv/stage.sh `
	--mount=type=bind,source=patches/50-soxr,target="${PREFIX}/patches" `
	/srv/build.sh

FROM layer-45 AS layer-50-svt-av1

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/50-svt-av1.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50-theora

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/50-theora.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50-vpx

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/50-vpx.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50-vulkan

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/50-vulkan/45-vulkan.sh,target=/srv/stage.sh `
	/srv/build.sh
RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/50-vulkan/50-shaderc.sh,target=/srv/stage.sh `
	/srv/build.sh
RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/50-vulkan/55-spirv-cross.sh,target=/srv/stage.sh `
	/srv/build.sh
RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/50-vulkan/60-placebo.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50-x264

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/50-x264.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50-x265

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/50-x265.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50-zimg

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/50-zimg.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50

COPY --from=layer-50-amf "${PREFIX}/." "$PREFIX"
COPY --from=layer-50-nvenc "${PREFIX}/." "$PREFIX"
COPY --from=layer-50-lame "${PREFIX}/." "$PREFIX"
COPY --from=layer-50-onevpl "${PREFIX}/." "$PREFIX"
COPY --from=layer-50-opus "${PREFIX}/." "$PREFIX"
COPY --from=layer-50-soxr "${PREFIX}/." "$PREFIX"
COPY --from=layer-50-svt-av1 "${PREFIX}/." "$PREFIX"
COPY --from=layer-50-theora "${PREFIX}/." "$PREFIX"
COPY --from=layer-50-vpx "${PREFIX}/." "$PREFIX"
COPY --from=layer-50-vulkan "${PREFIX}/." "$PREFIX"
COPY --from=layer-50-x264 "${PREFIX}/." "$PREFIX"
COPY --from=layer-50-x265 "${PREFIX}/." "$PREFIX"
COPY --from=layer-50-zimg "${PREFIX}/." "$PREFIX"

#--

FROM layer-45 AS layer-99-heif

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/99-heif.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-00 AS layer-99-protoc

ADD https://raw.githubusercontent.com/protocolbuffers/protobuf/v25.0/LICENSE '/srv/protoc/LICENSE'

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/99-protoc.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-00 AS layer-99-pdfium

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/99-pdfium.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-50 AS layer-99-ffmpeg

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=stages/99-ffmpeg.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-50 AS layer-99

COPY --from=layer-99-heif "${OUT}/." "$OUT"
COPY --from=layer-99-heif "${PREFIX}/srv/." "${OUT}/srv"
COPY --from=layer-99-heif "${PREFIX}/licenses/." "${OUT}/licenses"

COPY --from=layer-99-protoc "${OUT}/." "$OUT"
COPY --from=layer-99-protoc "${PREFIX}/licenses/." "${OUT}/licenses"

COPY --from=layer-99-pdfium "${OUT}/." "$OUT"
COPY --from=layer-99-pdfium "${PREFIX}/licenses/." "${OUT}/licenses"

COPY --from=layer-99-ffmpeg "${OUT}/." "$OUT"
COPY --from=layer-99-ffmpeg "${PREFIX}/srv/." "${OUT}/srv"
COPY --from=layer-99-ffmpeg "${PREFIX}/licenses/." "${OUT}/licenses"

RUN rm -rf "${OUT}/share" "${OUT}/lib/pkgconfig" "${OUT}/lib/cmake"

RUN { cd "${OUT}/bin" || exit 0; } `
	&& find . -name '*.lib' -exec install -Dt ../lib/ -m a-rwx,u+rw,g+r,o+r {} + `
	&& find . -not -name '*.dll' -delete `
	&& find ../lib -name '*.def' -delete

RUN find "$OUT" -type d -delete 2>/dev/null || true

RUN cd "${OUT}/srv" && env XZ_OPT='-T0 -7' bsdtar -cJf ../src.tar.xz *

RUN rm -rf "${OUT}/srv"

# Ensure all .so libs have the correct rpaths
RUN find "$OUT" -type f \( -name '*.so' -o -name '*.so.*' \) -exec patchelf --set-rpath '$ORIGIN' {} \;

# Strip debug symbols and ensure any .so, .dll, .dylib has the execution flag set
RUN --mount=type=cache,target=/root/.cache `
	echo 'strip -S "$@" && chmod +x "$@"' >/srv/stage.sh `
	&& find "$OUT" -type f \( -name '*.so' -o -name '*.so.*' -o -name '*.dll' -o -name '*.dylib' \) -exec /srv/build.sh {} +

# Symlink .lib to .dll.a
RUN find "$OUT/lib" -name '*.lib' -exec `
	sh -euxc 'for _file in "$@"; do ln -sf "$(basename "$_file")" "$(dirname "$_file")/lib$(basename "$_file" .lib).dll.a"; done' sh {} +

# Create macOS Frameworks from the built libs
RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/create-framework.sh,target=/srv/stage.sh `
	/srv/build.sh

#--

FROM scratch

ARG OUT

COPY --from=layer-99 "${OUT:?}/." /out
