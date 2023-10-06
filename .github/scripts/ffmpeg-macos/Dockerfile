ARG FAKE_DEPS="gettext-runtime libiconv ncurses" \
	FFMPEG_DEPS="brotli bzip2 dav1d libde265 libjxl libopus libpng libvorbis libvpx-devel openjpeg \
	soxr xz zimg" \
	LIBWEBP_VERSION=1.3.2 \
	FFMPEG_VERSION=6.0 \
	LIBHEIF_VERSION=1.16.2

FROM vvasconcellos/osxcross:12.3-564e2b9-8 as base

SHELL ["/bin/bash", "-eux", "-o", "pipefail", "-c"]

WORKDIR /srv

ARG LIBWEBP_VERSION
ADD "https://github.com/webmproject/libwebp/archive/refs/tags/v${LIBWEBP_VERSION}.tar.gz" ./
RUN tar -xf "v${LIBWEBP_VERSION}.tar.gz" && rm "v${LIBWEBP_VERSION}.tar.gz" \
	&& \
	mv "/srv/libwebp-${LIBWEBP_VERSION}" /srv/libwebp

ARG LIBHEIF_VERSION
ADD "https://github.com/strukturag/libheif/releases/download/v${LIBHEIF_VERSION}/libheif-${LIBHEIF_VERSION}.tar.gz" ./
RUN tar -xf "libheif-${LIBHEIF_VERSION}.tar.gz" && rm "libheif-${LIBHEIF_VERSION}.tar.gz" \
	&& \
	mv "/srv/libheif-${LIBHEIF_VERSION}" /srv/libheif

ARG FFMPEG_VERSION
ADD "https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VERSION}.tar.xz" ./
RUN tar -xf "ffmpeg-${FFMPEG_VERSION}.tar.xz" && rm "ffmpeg-${FFMPEG_VERSION}.tar.xz" \
	&& \
	mv "/srv/ffmpeg-${FFMPEG_VERSION}" /srv/ffmpeg \
	&& \
	cd /srv/ffmpeg \
	&& \
	for patch in \
	'https://github.com/macports/macports-ports/raw/0e62a6d66fbaa7faf7b4eb9029647d3d5651fb2e/multimedia/ffmpeg6/files/patch-libavcodec-audiotoolboxenc.c.diff' \
	'https://github.com/macports/macports-ports/raw/0e62a6d66fbaa7faf7b4eb9029647d3d5651fb2e/multimedia/ffmpeg6/files/patch-avutil-builtin-available.diff' \
	'https://github.com/macports/macports-ports/raw/0e62a6d66fbaa7faf7b4eb9029647d3d5651fb2e/multimedia/ffmpeg6/files/patch-libavcodec-profvidworkflow.diff' \
	; do curl -LSs "$patch" | patch -p0; done

# ---
FROM base as x86_64

# Fake Install macOS dependencies not required to build ffmpeg
ARG FAKE_DEPS
# hadolint ignore=SC2086
RUN osxcross-macports fake-install $FAKE_DEPS

# Install macOS dependencies required to build ffmpeg
ARG FFMPEG_DEPS
# hadolint ignore=SC2086
RUN --mount=type=cache,id=macports-x86_64,target=/opt/osxcross/macports/cache \
	osxcross-macports install $FFMPEG_DEPS

# Build ffmpeg
RUN --mount=src=build.sh,dst=/srv/build.sh /srv/build.sh x86_64 "$MACOSX_SDK"

# ---
FROM base as aarch64

# https://ffmpeg.org/pipermail/ffmpeg-user/2016-January/030202.html
ADD https://raw.githubusercontent.com/yuvi/gas-preprocessor/master/gas-preprocessor.pl /usr/local/bin/
RUN chmod +x /usr/local/bin/gas-preprocessor.pl

# Update min macOS version for arm64
# libbrotli macports precompiled binaries are only available for macOS 11.2+
ENV OSX_VERSION_MIN="11.2" \
	MACOSX_DEPLOYMENT_TARGET="11.2"

# Fake Install macOS dependencies not required to build ffmpeg
ARG FAKE_DEPS
# hadolint ignore=SC2086
RUN osxcross-macports fake-install --arm64 $FAKE_DEPS

# Install macOS dependencies required to build ffmpeg
ARG FFMPEG_DEPS
# hadolint ignore=SC2086
RUN --mount=type=cache,id=macports-arm64,target=/opt/osxcross/macports/cache \
	osxcross-macports install --arm64 $FFMPEG_DEPS

# Build ffmpeg
RUN --mount=src=build.sh,dst=/srv/build.sh /srv/build.sh aarch64 "$MACOSX_SDK"

# ---
FROM scratch

COPY --from=x86_64 /FFMpeg.framework /ffmpeg/x86_64/FFMpeg.framework
COPY --from=aarch64 /FFMpeg.framework /ffmpeg/aarch64/FFMpeg.framework
