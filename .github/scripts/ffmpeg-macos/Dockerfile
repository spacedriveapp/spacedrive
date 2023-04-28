# Darwin version is based on macOS version
# Check here when updating macOS version: https://github.com/tpoechtrager/osxcross/blob/master/build.sh#L31
ARG MACOS_VERSION=12.3 \
	DARWIN_VERSION=darwin21.4 \
	MACOS_MIN_VERSION=10.15 \
	FFMPEG_VERSION=6.0 \
	FFMEG_OUT_DIR=/ffmpeg

# Runtime macOS dependendies for ffmpeg
ARG MACPORT_DEPS="aom bzip2 fontconfig freetype fribidi lame libgsm libopus libtheora libvidstab libvorbis \
	libvpx-devel lzo2 openjpeg rav1e soxr svt-av1 webp x264 x265 XviD xz zimg zlib"

FROM alpine:3.17 as base

WORKDIR /srv

# Host dependencies, required to build osxcross, gcc for macOS and ffmpeg. ~1GiB
# hadolint ignore=DL3018
RUN --mount=type=cache,target=/var/cache/apk ln -vs /var/cache/apk /etc/apk/cache && apk add --update \
	bash bsd-compat-headers build-base bzip2-dev clang15 cmake curl git gmp-dev libc++-dev libc-dev libuuid libxml2-dev \
	llvm15-dev llvm15-static mpc1-dev mpfr-dev musl-fts-dev openssl openssl-dev perl python3 xz yasm zlib-dev

# Download osxcross, use a specific commit to avoid breaking changes and allow docker to cache it
ADD https://github.com/tpoechtrager/osxcross/archive/50e86ebca7d14372febd0af8cd098705049161b9.zip /srv/osxcross.zip
RUN unzip osxcross.zip && mv osxcross-* osxcross && rm osxcross.zip

WORKDIR /srv/osxcross/tarballs

# Download MacOS SDK
ARG MACOS_VERSION
ADD "https://github.com/joseluisq/macosx-sdks/releases/download/${MACOS_VERSION}/MacOSX${MACOS_VERSION}.sdk.tar.xz" ./

# Setupt osxcross environment variables
ARG MACOS_MIN_VERSION
ENV PATH="$PATH:/opt/osxcross/bin" \
	UNATTENDED=yes \
	OSXCROSS_MP_INC=1 \
	OSX_VERSION_MIN="$MACOS_MIN_VERSION" \
	MACOSX_DEPLOYMENT_TARGET="$MACOS_MIN_VERSION"

WORKDIR /srv/osxcross

# Some important unmerged PRs
COPY 180.diff 314.diff 379.diff ./
RUN set -eux; for patch in *.diff; do patch -p1 < "$patch"; done

# Build osxcross
RUN set -eux; export TARGET_DIR=/opt/osxcross \
	&& \
	./build.sh \
	&& \
	./build_compiler_rt.sh \
	&& \
	# Ugly workaround for linker not finding the macOS SDK's Framework directory
	ln -fs "${TARGET_DIR}/SDK/MacOSX${MACOS_VERSION}.sdk/System" '/System' \
	&& \
	./cleanup.sh

WORKDIR /srv

# Setup macports
RUN osxcross-macports --help

# ---
FROM base as ffmpeg

# Download ffmpeg
ARG FFMPEG_VERSION
ADD "https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VERSION}.tar.xz" ./
RUN tar -xf ffmpeg-${FFMPEG_VERSION}.tar.xz && rm ffmpeg-${FFMPEG_VERSION}.tar.xz

WORKDIR /srv/ffmpeg-${FFMPEG_VERSION}

COPY ./ffmpeg-build-macos.sh ./

# ---
FROM ffmpeg as x86_64

ARG MACPORT_DEPS

# Install macOS dependencies required to build ffmpeg
# hadolint ignore=SC2086
RUN osxcross-macports install $MACPORT_DEPS

ARG DARWIN_VERSION MACOS_VERSION FFMEG_OUT_DIR
RUN ./ffmpeg-build-macos.sh x86_64 "$DARWIN_VERSION" "$MACOS_VERSION" "$FFMEG_OUT_DIR"

# ---
FROM ffmpeg as aarch64

# https://ffmpeg.org/pipermail/ffmpeg-user/2016-January/030202.html
ADD https://raw.githubusercontent.com/yuvi/gas-preprocessor/master/gas-preprocessor.pl /usr/local/bin/
RUN chmod +x /usr/local/bin/gas-preprocessor.pl

# aarch64 requires a higher min macOS version to build ffmpeg
ARG MACPORT_DEPS MACOS_MIN_VERSION=11.0
ENV OSX_VERSION_MIN="$MACOS_MIN_VERSION" \
	MACOSX_DEPLOYMENT_TARGET="$MACOS_MIN_VERSION"

# Install macOS dependencies required to build ffmpeg
# hadolint ignore=SC2086
RUN osxcross-macports install --arm64 $MACPORT_DEPS

ARG DARWIN_VERSION MACOS_VERSION FFMEG_OUT_DIR
RUN ./ffmpeg-build-macos.sh aarch64 "$DARWIN_VERSION" "$MACOS_VERSION" "$FFMEG_OUT_DIR"

# ---
FROM scratch as ffmpeg-${FFMPEG_VERSION}

ARG FFMEG_OUT_DIR

COPY --from=x86_64 $FFMEG_OUT_DIR /ffmpeg/x86_64
COPY --from=aarch64 $FFMEG_OUT_DIR /ffmpeg/aarch64
