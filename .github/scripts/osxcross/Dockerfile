ARG MACOS_VERSION=12.3 \
	MACOS_MIN_VERSION=10.15 \
	# aarch64 requires a higher min macOS version to build ffmpeg
	ARM64_MACOS_MIN_VERSION=11.0

FROM alpine:3.17 as base

WORKDIR /srv

# Host dependencies, required to build osxcross, gcc for macOS and ffmpeg. ~1GiB
# hadolint ignore=DL3018
RUN --mount=type=cache,target=/var/cache/apk ln -vs /var/cache/apk /etc/apk/cache && apk add --update \
	bash bsd-compat-headers build-base bzip2-dev clang15 cmake curl git gmp-dev libc++-dev libc-dev libuuid libxml2-dev \
	llvm15-dev llvm15-static mpc1-dev mpfr-dev musl-fts-dev openssl openssl-dev perl python3 xz yasm zlib-dev

# Download osxcross, use a specific commit to avoid breaking changes and allow docker to cache it
ADD https://github.com/tpoechtrager/osxcross/archive/50e86eb.zip /srv/osxcross.zip
RUN unzip osxcross.zip && mv osxcross-* osxcross && rm osxcross.zip

WORKDIR /srv/osxcross/tarballs

# Download MacOS SDK
ARG MACOS_VERSION
ENV MACOSX_SDK="$MACOS_VERSION"
ADD "https://github.com/joseluisq/macosx-sdks/releases/download/${MACOS_VERSION}/MacOSX${MACOS_VERSION}.sdk.tar.xz" ./

# Setupt osxcross environment variables
ARG MACOS_MIN_VERSION ARM64_MACOS_MIN_VERSION
ENV PATH="$PATH:/opt/osxcross/bin" \
	UNATTENDED=yes \
	OSXCROSS_MP_INC=1 \
	OSX_VERSION_MIN="$MACOS_MIN_VERSION" \
	MACOSX_DEPLOYMENT_TARGET="$MACOS_MIN_VERSION" \
	MACOSX_ARM64_DEPLOYMENT_TARGET="$ARM64_MACOS_MIN_VERSION"

WORKDIR /srv/osxcross

# Some important patches from unmerged PRs
# PR 180 code needed to be updated to work with the latest osxcross
# 181 is not related to the 181 PR. It's just custom code that needed to be patched after 180 and before 379
COPY 180.diff 181.diff 314.diff 372.diff 379.diff ./
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

LABEL org.opencontainers.image.title="alpine-osxcross" \
	# Version is macOS SDK version + osxcross commit hash
	org.opencontainers.image.version="12.3-50e86eb" \
	org.opencontainers.image.authors="Vítor Vasconcellos <vasconcellos.dev@gmail.com>, Spacedrive <support@spacedrive.com>" \
	org.opencontainers.image.revision="1" \
	org.opencontainers.image.licenses="GPL-2.0" \
	org.opencontainers.image.description="macOS cross toolchain configured inside Alpine Linux"
