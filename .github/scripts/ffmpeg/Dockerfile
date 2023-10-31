# escape=`

ARG OUT="/opt/out"
ARG TARGET="x86_64-linux-gnu"
ARG ZIG_VERSION="0.11.0"
ARG MESON_VERSION="1.2.3"
ARG CMAKE_VERSION="3.27.0"

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

# Add LLVM 17 repository
ADD https://apt.llvm.org/llvm-snapshot.gpg.key /etc/apt/trusted.gpg.d/apt.llvm.org.asc

RUN chmod 644 /etc/apt/trusted.gpg.d/apt.llvm.org.asc

RUN echo "deb https://apt.llvm.org/bookworm/ llvm-toolchain-bookworm-17 main" `
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
	llvm-17 `
	autoconf `
	clang-17 `
	libc6-dev `
	pkg-config `
	ninja-build `
	libc++-17-dev `
	libc-devtools- `
	libc++abi-17-dev `
	libarchive-tools

# Configure sysroot and prefix
ARG OUT
ENV OUT="${OUT:?}"
ENV PREFIX="/opt/prefix"
ENV SYSROOT="/opt/sysroot"

ENV PATH="${SYSROOT}/bin:$PATH"

# Create required directories
RUN mkdir -p "$OUT" "${PREFIX}/bin" "${SYSROOT}/bin" "${SYSROOT}/wrapper"

# Utility to download, extract and cache archived files
ADD --chmod=0750 ./curl_tar.sh "${SYSROOT}/bin/curl_tar"

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

# Symlink some LLVM tools to their generic names
RUN ln -s "$(command -v clang-17)" "${SYSROOT}/bin/clang" `
	&& ln -s "$(command -v llvm-nm-17)" "${SYSROOT}/bin/nm" `
	&& ln -s "$(command -v llvm-strip-17)" "${SYSROOT}/bin/strip" `
	&& ln -s "$(command -v llvm-readelf-17)" "${SYSROOT}/bin/readelf" `
	&& ln -s "$(command -v llvm-objdump-17)" "${SYSROOT}/bin/objdump"

# Create a llvm rc wrapper script with some pre-configurations
# Do not name this llvm-rc or rc.exe, to avoid cmake weird special behavior for those tool names
RUN echo '#!/usr/bin/env sh' >"${SYSROOT}/bin/rc" `
	&& echo exec $(command -v llvm-rc-17) /I "${SYSROOT}/lib/libc/include/any-windows-any" '"$@"' >>"${SYSROOT}/bin/rc" `
	&& chmod +x "${SYSROOT}/bin/rc"

# Ensure no one tries to call the native system linker
RUN ln -s '/usr/bin/false' "${SYSROOT}/bin/ld"

# Add wrapper script for zig compilers, we need to ensure that they are called with the correct arguments
ADD ./zig-cc.sh "${SYSROOT}/bin/zig-cc"
RUN ln -s 'zig-cc' "${SYSROOT}/bin/zig-c++"
RUN chmod +x "${SYSROOT}/bin/zig-cc" "${SYSROOT}/bin/zig-c++"

# Create script for zig ar command, because cmake doesn't allow passing arguments to tools
RUN echo '#!/usr/bin/env sh' >"${SYSROOT}/bin/ar"
RUN echo 'exec zig ar "$@"' >>"${SYSROOT}/bin/ar"
RUN chmod +x "${SYSROOT}/bin/ar"

# Create script for zig ranlib command, because cmake doesn't allow passing arguments to tools
RUN echo '#!/usr/bin/env sh' >"${SYSROOT}/bin/ranlib"
RUN echo 'exec zig ranlib "$@"' >>"${SYSROOT}/bin/ranlib"
RUN chmod +x "${SYSROOT}/bin/ranlib"

# Create script for zig dlltool command, so cmake can find it
RUN echo '#!/usr/bin/env sh' >"${SYSROOT}/bin/dlltool"
RUN echo 'exec zig dlltool "$@"' >>"${SYSROOT}/bin/dlltool"
RUN chmod +x "${SYSROOT}/bin/dlltool"

# Create script for zig objcopy command, because cmake doesn't allow passing arguments to tools
RUN echo '#!/usr/bin/env sh' >"${SYSROOT}/bin/objcopy"
RUN echo 'exec zig objcopy "$@"' >>"${SYSROOT}/bin/objcopy"
RUN chmod +x "${SYSROOT}/bin/objcopy"

# Create a cmake wrapper script with some pre-configurations
RUN echo '#!/usr/bin/env sh' >"${SYSROOT}/wrapper/cmake"
RUN echo exec "$(command -v cmake)" `
	-GNinja `
	-Wno-dev `
	-DCMAKE_TOOLCHAIN_FILE=/srv/toolchain.cmake `
	-DCMAKE_BUILD_TYPE=MinSizeRel `
	-DBUILD_SHARED_LIBS="\${SHARED:-Off}" `
	-DCMAKE_INSTALL_PREFIX="\${PREFIX}" `
	-DCMAKE_BUILD_WITH_INSTALL_RPATH=On `
	-DCMAKE_POSITION_INDEPENDENT_CODE=On `
	-DCMAKE_INTERPROCEDURAL_OPTIMIZATION=\${LTO:-On} `
	'"$@"' >>"${SYSROOT}/wrapper/cmake"
RUN chmod +x "${SYSROOT}/wrapper/cmake"

# Create a meson wrapper script with some pre-configurations
RUN echo '#!/usr/bin/env sh' >"${SYSROOT}/wrapper/meson"
RUN echo exec "$(command -v meson)" setup `
	--prefix="\${PREFIX}" `
	--buildtype=minsize `
	--cross-file=/srv/cross.meson `
	--default-library="\${SHARED:-static}" `
	-Db_lto=\${LTO:-true} `
	-Db_staticpic=true `
	'"$@"' >>"${SYSROOT}/wrapper/meson"
RUN chmod +x "${SYSROOT}/wrapper/meson"

# Create cmake and meson toolchain files
ARG TARGET
ENV TARGET="${TARGET:?}"
RUN --mount=type=bind,source=toolchain.sh,target=/srv/toolchain.sh /srv/toolchain.sh

# Wrapper script that defines autotools and configure flags
COPY ./build.sh /srv/build.sh

# Cache cmake system information discovery, which takes an uncesseary amount of time to run
RUN --mount=type=cache,target=/root/.cache `
	echo 'cmake --system-information >/dev/null' > /srv/stage.sh && /srv/build.sh

#--

FROM build-base AS layer-10-sse2neon

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/10-sse2neon.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM build-base AS layer-10

COPY --from=layer-10-sse2neon "${PREFIX}/." $PREFIX

#--

FROM layer-10 AS layer-20-brotli

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/20-brotli.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-10 AS layer-20-bzip2

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/20-bzip2.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-10 AS layer-20-zlib

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/20-zlib.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-10 AS layer-20

COPY --from=layer-20-brotli "${PREFIX}/." $PREFIX
COPY --from=layer-20-bzip2 "${PREFIX}/." $PREFIX
COPY --from=layer-20-zlib "${PREFIX}/." $PREFIX

#--

FROM layer-20 AS layer-25-lcms

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/25-lcms.sh,target=/srv/stage.sh `
	--mount=type=bind,source=patches/25-lcms,target=${PREFIX}/patches `
	/srv/build.sh

FROM layer-20 AS layer-25-lzma

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/25-lzma.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-20 AS layer-25-ogg

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/25-ogg.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-20 AS layer-25

COPY --from=layer-25-lcms "${PREFIX}/." $PREFIX
COPY --from=layer-25-lzma "${PREFIX}/." $PREFIX
COPY --from=layer-25-ogg "${PREFIX}/." $PREFIX

#--

FROM layer-25 AS layer-45-opencl

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/45-opencl/25-opencl-headers.sh,target=/srv/stage.sh `
	/srv/build.sh

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/45-opencl/45-opencl.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-25 AS layer-45-vorbis

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/45-vorbis.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-25 AS layer-45

COPY --from=layer-45-opencl "${PREFIX}/." $PREFIX
COPY --from=layer-45-vorbis "${PREFIX}/." $PREFIX

#--

FROM layer-45 AS layer-50-amf

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/50-amf.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50-dav1d

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/50-dav1d.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50-de265

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/50-de265.sh,target=/srv/stage.sh `
	--mount=type=bind,source=patches/50-de265,target=${PREFIX}/patches `
	/srv/build.sh

FROM layer-45 AS layer-50-nvenc

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/50-nvenc.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50-lame

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/50-lame.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50-onevpl

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/50-onevpl.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50-opus

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/50-opus.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50-sharpyuv

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/50-sharpyuv.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50-soxr

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/50-soxr.sh,target=/srv/stage.sh `
	--mount=type=bind,source=patches/50-soxr,target=${PREFIX}/patches `
	/srv/build.sh

FROM layer-45 AS layer-50-svt-av1

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/50-svt-av1.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50-theora

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/50-theora.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50-vpx

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/50-vpx.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50-vulkan

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/50-vulkan/45-vulkan.sh,target=/srv/stage.sh `
	/srv/build.sh
RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/50-vulkan/50-shaderc.sh,target=/srv/stage.sh `
	/srv/build.sh
RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/50-vulkan/55-spirv-cross.sh,target=/srv/stage.sh `
	/srv/build.sh
RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/50-vulkan/60-placebo.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50-x264

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/50-x264.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50-x265

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/50-x265.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50-zimg

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/50-zimg.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-45 AS layer-50

COPY --from=layer-50-amf "${PREFIX}/." $PREFIX
COPY --from=layer-50-dav1d "${PREFIX}/." $PREFIX
COPY --from=layer-50-de265 "${PREFIX}/." $PREFIX
COPY --from=layer-50-nvenc "${PREFIX}/." $PREFIX
COPY --from=layer-50-lame "${PREFIX}/." $PREFIX
COPY --from=layer-50-onevpl "${PREFIX}/." $PREFIX
COPY --from=layer-50-opus "${PREFIX}/." $PREFIX
COPY --from=layer-50-sharpyuv "${PREFIX}/." $PREFIX
COPY --from=layer-50-soxr "${PREFIX}/." $PREFIX
COPY --from=layer-50-svt-av1 "${PREFIX}/." $PREFIX
COPY --from=layer-50-theora "${PREFIX}/." $PREFIX
COPY --from=layer-50-vpx "${PREFIX}/." $PREFIX
COPY --from=layer-50-vulkan "${PREFIX}/." $PREFIX
COPY --from=layer-50-x264 "${PREFIX}/." $PREFIX
COPY --from=layer-50-x265 "${PREFIX}/." $PREFIX
COPY --from=layer-50-zimg "${PREFIX}/." $PREFIX

#--

FROM layer-50 AS layer-99-heif

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/99-heif.sh,target=/srv/stage.sh `
	/srv/build.sh


FROM layer-50 AS layer-99-pdfium

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/99-pdfium.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-50 AS layer-99-ffmpeg

RUN --mount=type=cache,target=/root/.cache `
	--mount=type=bind,source=scripts/99-ffmpeg.sh,target=/srv/stage.sh `
	/srv/build.sh

FROM layer-50 AS layer-99

COPY --from=layer-99-heif "${OUT}/." "$OUT"
COPY --from=layer-99-heif "${PREFIX}/srv" "${OUT}/srv"
COPY --from=layer-99-heif "${PREFIX}/licenses" "${OUT}/licenses"

COPY --from=layer-99-pdfium "${OUT}/." "$OUT"
COPY --from=layer-99-pdfium "${PREFIX}/srv" "${OUT}/srv"
COPY --from=layer-99-pdfium "${PREFIX}/licenses" "${OUT}/licenses"

COPY --from=layer-99-ffmpeg "${OUT}/." "$OUT"
COPY --from=layer-99-ffmpeg "${PREFIX}/srv" "${OUT}/srv"
COPY --from=layer-99-ffmpeg "${PREFIX}/licenses" "${OUT}/licenses"

RUN rm -rf "${OUT}/share" "${OUT}/lib/pkgconfig" "${OUT}/lib/cmake"

RUN { cd "${OUT}/bin" || exit 0; } `
	&& find . -name '*.lib' -exec install -Dt ../lib/ -m a-rwx,u+rw,g+r,o+r {} + `
	&& find . -not -name '*.dll' -delete `
	&& find ../lib -name '*.def' -delete

RUN find "$OUT" -type d -delete || true

# Strip debug symbols and ensure any .so, .dll, .dylib has the execution flag set
RUN find "$OUT" -type f \( -name '*.so' -o -name '*.so.*' -o -name '*.dll' -o -name '*.dylib' \) -exec `
	sh -euxc 'llvm-strip-17 -S "$@" && chmod +x "$@"' sh {} +

# Symlink .lib to .dll.a
RUN find "$OUT/lib" -name '*.lib' -exec `
	sh -euxc 'for _file in "$@"; do ln -sf "$(basename "$_file")" "$(dirname "$_file")/lib$(basename "$_file" .lib).dll.a"; done' sh {} +

FROM scratch

ARG OUT

COPY --from=layer-99 "${OUT:?}" /out
