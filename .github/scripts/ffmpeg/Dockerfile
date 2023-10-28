# escape=`

ARG TARGET="x86_64-linux-gnu"
ARG ZIG_VERSION="0.11.0"
ARG MESON_VERSION="1.2.3"
ARG CMAKE_VERSION="3.27.0"

#--

FROM debian:bookworm as base

ADD https://gist.githubusercontent.com/HeavenVolkoff/ff7b77b9087f956b8df944772e93c071/raw `
	/etc/apt/apt.conf.d/99docker-apt-config

RUN echo 'debconf debconf/frontend select Noninteractive' | debconf-set-selections

RUN rm -f /etc/apt/apt.conf.d/docker-clean

RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt `
	apt-get update && apt-get upgrade

#--

FROM base as build-base

# Install build dependencies
RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt `
	apt-get install `
	nasm `
	curl `
	make `
	clang `
	patch `
	libtool `
	python3 `
	gettext `
	autoconf `
	pkg-config `
	ninja-build `
	libc-devtools- `
	libarchive-tools

# Configure sysroot and prefix
ENV PREFIX="/srv/prefix"
ENV SYSROOT="/opt/sysroot"
ENV CARGO_HOME="/usr/local/cargo"

ENV PATH="${SYSROOT}/bin:$PATH"

# Create required directories
RUN mkdir -p "/opt/out" "${PREFIX}/bin" "${PREFIX}/wrapper" "${SYSROOT}/bin"

# Download and configure zig toolchain
ARG TARGET ZIG_VERSION
# Windows requires a newer zig due to resource compiler not being included in 0.11.0
RUN case "$TARGET" in *windows-gnu) ZIG_VERSION='0.12.0-dev.1303+8f4853369' ;; esac `
	&& curl -LSs "https://ziglang.org/builds/zig-linux-$(uname -m)-${ZIG_VERSION}.tar.xz" `
	| bsdtar -xf- --strip-component 1 -C "$SYSROOT"
RUN mv "${SYSROOT}/zig" "${SYSROOT}/bin/zig"

# Download and install cmake
ARG CMAKE_VERSION
RUN curl -LSso /usr/local/cmake.sh `
	"https://github.com/Kitware/CMake/releases/download/v${CMAKE_VERSION}/cmake-${CMAKE_VERSION}-linux-$(uname -m).sh" `
	&& chmod +x /usr/local/cmake.sh `
	&& /usr/local/cmake.sh --exclude-subdir --skip-license --prefix=/usr/local `
	&& rm /usr/local/cmake.sh

# Download and install meson, with a patch to add zig support
ARG MESON_VERSION
RUN mkdir /srv/meson `
	&& curl -LSs "https://github.com/mesonbuild/meson/archive/refs/tags/${MESON_VERSION}.tar.gz" `
	| bsdtar -xf- --strip-component 1 -C /srv/meson `
	&& cd /srv/meson `
	&& curl -LSs 'https://github.com/mesonbuild/meson/pull/12293.patch' | patch -p1 `
	&& ./packaging/create_zipapp.py --outfile "${_sysroot}/bin/meson" --compress `
	&& rm -rf /srv/meson

# Install gas-preprocessor, used by ffmpeg to handle assembly files
RUN curl -LSso "${_sysroot}/bin/gas-preprocessor.pl" https://github.com/libav/gas-preprocessor/raw/master/gas-preprocessor.pl `
	&& chmod +x "${_sysroot}/bin/gas-preprocessor.pl"

# Workaround for -lsynchronization linking issue on Windows GNU targets
# https://github.com/ziglang/zig/issues/14919
RUN cd "${SYSROOT}/lib/libc/mingw/lib-common" `
	&& { [ -f 'synchronization.def' ] || ln -s 'api-ms-win-core-synch-l1-2-0.def' 'synchronization.def'; }

# Add wrapper command for zig compiler and linker, we need to ensure that they are called with the correct arguments
ADD ./zig-cc.sh "${SYSROOT}/bin/zig-cc"
RUN ln -s 'zig-cc' "${SYSROOT}/bin/zig-c++"
RUN ln -s 'zig-cc' "${SYSROOT}/bin/zig-ld"
RUN chmod +x "${SYSROOT}/bin/zig-cc" "${SYSROOT}/bin/zig-c++" "${SYSROOT}/bin/zig-ld"

# Add wrapper command for zig rc, we need to shim some behaviour that cmake and meson expect from rc.exe
ADD ./zig-rc.sh "${SYSROOT}/bin/zig-rc"
RUN chmod +x "${SYSROOT}/bin/zig-rc"

# Create script for zig ar command, because cmake doesn't allow passing arguments to tools
RUN echo '#!/usr/bin/env sh' >"${SYSROOT}/bin/zig-ar"
RUN echo 'exec zig ar "$@"' >>"${SYSROOT}/bin/zig-ar"
RUN chmod +x "${SYSROOT}/bin/zig-ar"

# Create script for zig ranlib command, because cmake doesn't allow passing arguments to tools
RUN echo '#!/usr/bin/env sh' >"${SYSROOT}/bin/zig-ranlib"
RUN echo 'exec zig ranlib "$@"' >>"${SYSROOT}/bin/zig-ranlib"
RUN chmod +x "${SYSROOT}/bin/zig-ranlib"

# Create script for zig dlltool command, so cmake can find it
RUN echo '#!/usr/bin/env sh' >"${SYSROOT}/bin/dlltool"
RUN echo 'exec zig dlltool "$@"' >>"${SYSROOT}/bin/dlltool"
RUN chmod +x "${SYSROOT}/bin/dlltool"

# Create a cmake wrapper script with some pre-configurations
RUN echo '#!/usr/bin/env sh' >"${PREFIX}/wrapper/cmake"
RUN echo exec "$(command -v cmake)" `
	-GNinja `
	-Wno-dev `
	-DCMAKE_TOOLCHAIN_FILE=/srv/toolchain.cmake `
	-DCMAKE_BUILD_TYPE=MinSizeRel `
	-DBUILD_SHARED_LIBS="\${SHARED:-Off}" `
	-DCMAKE_INSTALL_PREFIX="\${PREFIX}" `
	-DCMAKE_BUILD_WITH_INSTALL_RPATH=On `
	-DCMAKE_INTERPROCEDURAL_OPTIMIZATION=On `
	'"$@"' >>"${PREFIX}/wrapper/cmake"
RUN chmod +x "${PREFIX}/wrapper/cmake"

# Create a meson wrapper script with some pre-configurations
RUN echo '#!/usr/bin/env sh' >"${PREFIX}/wrapper/meson"
RUN echo exec "$(command -v meson)" setup `
	--prefix="\${PREFIX}" `
	--buildtype=minsize `
	--cross-file=/srv/cross.meson `
	--default-library="\${SHARED:-static}" `
	-Db_lto=true `
	'"$@"' >>"${PREFIX}/wrapper/meson"
RUN chmod +x "${PREFIX}/wrapper/meson"

# Create cmake and meson toolchain files
ENV TARGET="$TARGET"
RUN --mount=src=toolchain.sh,dst=/srv/toolchain.sh /srv/toolchain.sh

# Wrapper script that defines autotools and configure flags
COPY ./build.sh /srv/build.sh

# Cache cmake system information discovery, which takes an uncesseary amount of time to run
RUN echo 'cmake --system-information >/dev/null' > /srv/stage.sh && /srv/build.sh

#--

FROM build-base AS layer-10-sse2neon

RUN --mount=src=scripts/10-sse2neon.sh,dst=/srv/stage.sh /srv/build.sh

FROM build-base AS layer-10

COPY --from=layer-10-sse2neon $PREFIX/. $PREFIX

#--

FROM layer-10 AS layer-20-brotli

RUN --mount=src=scripts/20-brotli.sh,dst=/srv/stage.sh /srv/build.sh

FROM layer-10 AS layer-20-bzip2

RUN --mount=src=scripts/20-bzip2.sh,dst=/srv/stage.sh /srv/build.sh

FROM layer-10 AS layer-20-zlib

RUN --mount=src=scripts/20-zlib.sh,dst=/srv/stage.sh /srv/build.sh

FROM layer-10 AS layer-20

COPY --from=layer-20-brotli $PREFIX/. $PREFIX
COPY --from=layer-20-bzip2 $PREFIX/. $PREFIX
COPY --from=layer-20-zlib $PREFIX/. $PREFIX

#--

FROM layer-20 AS layer-25-lcms

RUN `
	--mount=src=scripts/25-lcms.sh,dst=/srv/stage.sh `
	--mount=src=patches/25-lcms,dst=${PREFIX}/patches `
	/srv/build.sh

FROM layer-20 AS layer-25-lzma

RUN --mount=src=scripts/25-lzma.sh,dst=/srv/stage.sh /srv/build.sh

FROM layer-20 AS layer-25-ogg

RUN --mount=src=scripts/25-ogg.sh,dst=/srv/stage.sh /srv/build.sh

FROM layer-20 AS layer-25

COPY --from=layer-25-lcms $PREFIX/. $PREFIX
COPY --from=layer-25-lzma $PREFIX/. $PREFIX
COPY --from=layer-25-ogg $PREFIX/. $PREFIX

#--

FROM layer-25 AS layer-45-opencl

RUN --mount=src=scripts/45-opencl.sh,dst=/srv/stage.sh /srv/build.sh

FROM layer-25 AS layer-45-vorbis

RUN --mount=src=scripts/45-vorbis.sh,dst=/srv/stage.sh /srv/build.sh

FROM layer-25 AS layer-45

COPY --from=layer-45-opencl $PREFIX/. $PREFIX
COPY --from=layer-45-vorbis $PREFIX/. $PREFIX

#--

FROM layer-45 AS layer-50-amf

RUN --mount=src=scripts/50-amf.sh,dst=/srv/stage.sh /srv/build.sh

FROM layer-45 AS layer-50-dav1d

RUN --mount=src=scripts/50-dav1d.sh,dst=/srv/stage.sh /srv/build.sh

FROM layer-45 AS layer-50-de265

RUN `
	--mount=src=scripts/50-de265.sh,dst=/srv/stage.sh `
	--mount=src=patches/50-de265,dst=${PREFIX}/patches `
	/srv/build.sh

FROM layer-45 AS layer-50-nvenc

RUN --mount=src=scripts/50-nvenc.sh,dst=/srv/stage.sh /srv/build.sh

FROM layer-45 AS layer-50-lame

RUN --mount=src=scripts/50-lame.sh,dst=/srv/stage.sh /srv/build.sh

FROM layer-45 AS layer-50-onevpl

RUN --mount=src=scripts/50-onevpl.sh,dst=/srv/stage.sh /srv/build.sh

FROM layer-45 AS layer-50-opus

RUN --mount=src=scripts/50-opus.sh,dst=/srv/stage.sh /srv/build.sh

FROM layer-45 AS layer-50-sharpyuv

RUN --mount=src=scripts/50-sharpyuv.sh,dst=/srv/stage.sh /srv/build.sh

FROM layer-45 AS layer-50-soxr

RUN `
	--mount=src=scripts/50-soxr.sh,dst=/srv/stage.sh `
	--mount=src=patches/50-soxr,dst=${PREFIX}/patches `
	/srv/build.sh

FROM layer-45 AS layer-50-svt-av1

RUN --mount=src=scripts/50-svt-av1.sh,dst=/srv/stage.sh /srv/build.sh

FROM layer-45 AS layer-50-theora

RUN --mount=src=scripts/50-theora.sh,dst=/srv/stage.sh /srv/build.sh

FROM layer-45 AS layer-50-vpx

RUN --mount=src=scripts/50-vpx.sh,dst=/srv/stage.sh /srv/build.sh

FROM layer-45 AS layer-50-vulkan

RUN --mount=src=scripts/50-vulkan/45-vulkan.sh,dst=/srv/stage.sh /srv/build.sh
RUN --mount=src=scripts/50-vulkan/50-shaderc.sh,dst=/srv/stage.sh /srv/build.sh
RUN --mount=src=scripts/50-vulkan/55-spirv-cross.sh,dst=/srv/stage.sh /srv/build.sh
RUN --mount=src=scripts/50-vulkan/60-placebo.sh,dst=/srv/stage.sh /srv/build.sh

FROM layer-45 AS layer-50-x264

RUN --mount=src=scripts/50-x264.sh,dst=/srv/stage.sh /srv/build.sh

FROM layer-45 AS layer-50-x265

RUN --mount=src=scripts/50-x265.sh,dst=/srv/stage.sh /srv/build.sh

FROM layer-45 AS layer-50-zimg

RUN --mount=src=scripts/50-zimg.sh,dst=/srv/stage.sh /srv/build.sh

FROM layer-45 AS layer-50

COPY --from=layer-50-amf $PREFIX/. $PREFIX
COPY --from=layer-50-dav1d $PREFIX/. $PREFIX
COPY --from=layer-50-de265 $PREFIX/. $PREFIX
COPY --from=layer-50-nvenc $PREFIX/. $PREFIX
COPY --from=layer-50-lame $PREFIX/. $PREFIX
COPY --from=layer-50-onevpl $PREFIX/. $PREFIX
COPY --from=layer-50-opus $PREFIX/. $PREFIX
COPY --from=layer-50-sharpyuv $PREFIX/. $PREFIX
COPY --from=layer-50-soxr $PREFIX/. $PREFIX
COPY --from=layer-50-svt-av1 $PREFIX/. $PREFIX
COPY --from=layer-50-theora $PREFIX/. $PREFIX
COPY --from=layer-50-vpx $PREFIX/. $PREFIX
COPY --from=layer-50-vulkan $PREFIX/. $PREFIX
COPY --from=layer-50-x264 $PREFIX/. $PREFIX
COPY --from=layer-50-x265 $PREFIX/. $PREFIX
COPY --from=layer-50-zimg $PREFIX/. $PREFIX

#--

FROM layer-50 AS layer-99-ffmpeg

RUN --mount=src=scripts/99-ffmpeg.sh,dst=/srv/stage.sh /srv/build.sh

FROM layer-50 AS layer-99-heif

RUN --mount=src=scripts/99-heif.sh,dst=/srv/stage.sh /srv/build.sh

FROM layer-50 AS layer-99

COPY --from=layer-99-heif /opt/out /opt/out
COPY --from=layer-99-heif $PREFIX/licenses /opt/out/licenses

COPY --from=layer-99-ffmpeg /opt/out /opt/out
COPY --from=layer-99-ffmpeg $PREFIX/licenses /opt/out/licenses

RUN rm -rf /opt/out/bin /opt/out/share /opt/out/lib/pkgconfig /opt/out/lib/cmake

RUN { cd /opt/out/bin || exit 0; } `
	&& find . -name '*.lib' -exec install -Dt ../lib/ -m a-rwx,u+rw,g+r,o+r {} + `
	&& find . -name '*.lib' -delete `
	&& find . -name '*.dll' -exec x86_64-w64-mingw32-strip -s {} \;

RUN find /opt/out -type d -delete || true

FROM scratch

COPY --from=layer-99 /opt/out /out
