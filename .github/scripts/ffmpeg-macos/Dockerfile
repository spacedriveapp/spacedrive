# Darwin version is based on macOS version
# Check here when updating macOS version: https://github.com/tpoechtrager/osxcross/blob/master/build.sh#L31
ARG MACOS_VERSION=12.3 \
	DARWIN_VERSION=darwin21.4 \
	MACOS_MIN_VERSION=10.15 \
	FFMPEG_VERSION=6.0 \
	FFMEG_OUT_DIR=/ffmpeg

ARG MACPORT_DEPS="bzip2 lzo2 xz zimg zlib libgsm aom x264 XviD x265 soxr webp rav1e lame svt-av1 \
libvidstab libopus openjpeg libvorbis libtheora fontconfig freetype fribidi libvpx-devel"

FROM alpine:3.17 as base

WORKDIR /srv

# 1037 MiB
RUN apk --no-cache add xz git perl curl bash cmake clang15 yasm python3 \
	libuuid openssl build-base gmp-dev mpfr-dev mpc1-dev libc-dev libc++-dev \
	zlib-dev musl-fts-dev bzip2-dev llvm15-dev llvm15-static libxml2-dev \
	openssl-dev bsd-compat-headers

# Make clang 15 accessible without the version suffix
RUN ln -s $(command -v clang-15) /usr/local/bin/clang
RUN ln -s $(command -v clang++-15) /usr/local/bin/clang++

# Download osxcross
ADD https://github.com/tpoechtrager/osxcross/archive/50e86ebca7d14372febd0af8cd098705049161b9.zip /srv/osxcross.zip
RUN unzip osxcross.zip && mv osxcross-* osxcross && rm osxcross.zip

WORKDIR /srv/osxcross/tarballs

# Download MacOS SDK
ARG MACOS_VERSION
ADD "https://github.com/joseluisq/macosx-sdks/releases/download/${MACOS_VERSION}/MacOSX${MACOS_VERSION}.sdk.tar.xz" ./

# Build osxcross
WORKDIR /srv/osxcross

ARG MACOS_MIN_VERSION
ENV UNATTENDED=yes \
	OSXCROSS_MP_INC=1 \
	OSX_VERSION_MIN="$MACOS_MIN_VERSION" \
	MACOSX_DEPLOYMENT_TARGET="$MACOS_MIN_VERSION"

RUN ./build.sh
RUN ./build_compiler_rt.sh

# Add osxcross tools to PATH
ENV PATH="$PATH:/srv/osxcross/target/bin"

# Ugly workaround for linker not finding the SDK Framework
RUN ln -fs "/srv/osxcross/target/SDK/MacOSX${MACOS_VERSION}.sdk/System" '/System'

# Setup macports
RUN osxcross-macports --help

WORKDIR /srv

# Download ffmpeg
ARG FFMPEG_VERSION
ADD "https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VERSION}.tar.xz" ./
RUN tar -xf ffmpeg-${FFMPEG_VERSION}.tar.xz && rm ffmpeg-${FFMPEG_VERSION}.tar.xz

WORKDIR /srv/ffmpeg-${FFMPEG_VERSION}

COPY ./ffmpeg-build-macos.sh ./

# ---
from base as x86_64

ARG DARWIN_VERSION MACOS_VERSION FFMEG_OUT_DIR MACPORT_DEPS

RUN osxcross-macports install $MACPORT_DEPS nasm

RUN ./ffmpeg-build-macos.sh x86_64 $DARWIN_VERSION $MACOS_VERSION $FFMEG_OUT_DIR

# ---
from base as aarch64

ARG DARWIN_VERSION MACOS_VERSION FFMEG_OUT_DIR MACPORT_DEPS

# https://ffmpeg.org/pipermail/ffmpeg-user/2016-January/030202.html
ADD https://raw.githubusercontent.com/yuvi/gas-preprocessor/master/gas-preprocessor.pl /usr/local/bin/
RUN chmod +x /usr/local/bin/gas-preprocessor.pl

# aarch64 requires a higher macOS version to build ffmpeg
ARG MACOS_MIN_VERSION=11.0
ENV OSX_VERSION_MIN="$MACOS_MIN_VERSION" \
	MACOSX_DEPLOYMENT_TARGET="$MACOS_MIN_VERSION"

RUN osxcross-macports install --arm64 $MACPORT_DEPS

RUN ./ffmpeg-build-macos.sh aarch64 $DARWIN_VERSION $MACOS_VERSION $FFMEG_OUT_DIR

# ---
FROM scratch as ffmpeg-${FFMPEG_VERSION}

ARG FFMEG_OUT_DIR

COPY --from=x86_64 $FFMEG_OUT_DIR /ffmpeg/x86_64
COPY --from=aarch64 $FFMEG_OUT_DIR /ffmpeg/aarch64
