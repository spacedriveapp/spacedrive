ARG DEBIAN_FRONTEND=noninteractive

FROM debian:bookworm as base

ARG DEBIAN_FRONTEND

ADD https://gist.githubusercontent.com/HeavenVolkoff/ff7b77b9087f956b8df944772e93c071/raw \
	/etc/apt/apt.conf.d/99docker-apt-config

RUN rm -f /etc/apt/apt.conf.d/docker-clean; \
	echo 'Binary::apt::APT::Keep-Downloaded-Packages "true";' > /etc/apt/apt.conf.d/keep-cache

RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt \
	apt-get update && apt-get upgrade

FROM base as mingw

ARG DEBIAN_FRONTEND

RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt \
	apt-get install -y \
	bison build-essential bzip2 cvs flex git gzip m4 pax subversion texinfo wget xz-utils yasm

WORKDIR /srv

ADD --chmod=755 https://raw.githubusercontent.com/rdp/ffmpeg-windows-build-helpers/9b0e37c6ca24e16a877926691129a5e486d69974/patches/mingw-w64-build-r22.local \
	./

RUN ./mingw-w64-build-r22.local --gcc-ver=10.2.0 --mingw-w64-ver=9.0.0 \
	--default-configure --cpu-count="$(nproc)" --disable-shared --clean-build \
	--verbose --allow-overwrite --threads=pthreads-w32 --build-type=win64

FROM base

ARG DEBIAN_FRONTEND

COPY --from=mingw /srv/mingw-w64-x86_64 /opt/mingw-w64-x86_64

RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt \
	apt-get install -y \
	autoconf autoconf-archive autogen automake bison build-essential cmake curl git gperf \
	libarchive-tools libtool meson nasm patch pkg-config ragel yasm

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- \
	-yq \
	--no-modify-path \
	--default-toolchain none

ENV PATH="/opt/mingw-w64-x86_64/bin:/root/.cargo/bin:$PATH"

LABEL org.opencontainers.image.title="mingw" \
	# Version is GCC version + MinGW version
	org.opencontainers.image.version="10.2_9.0" \
	org.opencontainers.image.authors="Vítor Vasconcellos <vasconcellos.dev@gmail.com>, Spacedrive <support@spacedrive.com>" \
	org.opencontainers.image.revision="1" \
	org.opencontainers.image.licenses="GPL-2.0" \
	org.opencontainers.image.description="windows cross toolchain configured inside Debian Linux"
