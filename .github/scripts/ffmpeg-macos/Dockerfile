ARG FFMPEG_DEPS="aom bzip2 fontconfig freetype fribidi lame libgsm libopus libtheora libvidstab \
	libvorbis libvpx-devel lzo2 openjpeg rav1e soxr svt-av1 webp x264 x265 XviD xz zimg zlib" \
	FFMEG_OUT_DIR=/ffmpeg \
	FFMPEG_VERSION=6.0

FROM vvasconcellos/osxcross:12.3-50e86eb-1 as base

# Download ffmpeg
ARG FFMPEG_VERSION
ADD "https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VERSION}.tar.xz" ./
RUN tar -xf ffmpeg-${FFMPEG_VERSION}.tar.xz && rm ffmpeg-${FFMPEG_VERSION}.tar.xz

WORKDIR /srv/ffmpeg-${FFMPEG_VERSION}

COPY ./ffmpeg-build-macos.sh ./

# ---
FROM base as x86_64

# Install macOS dependencies required to build ffmpeg
ARG FFMPEG_DEPS
# hadolint ignore=SC2086
RUN --mount=type=cache,id=macports-x86_64,target=/opt/osxcross/macports/cache \
	osxcross-macports install $FFMPEG_DEPS

ARG FFMEG_OUT_DIR
RUN ./ffmpeg-build-macos.sh x86_64 "$MACOSX_SDK" "$FFMEG_OUT_DIR"

# ---
FROM base as aarch64

# https://ffmpeg.org/pipermail/ffmpeg-user/2016-January/030202.html
ADD https://raw.githubusercontent.com/yuvi/gas-preprocessor/master/gas-preprocessor.pl /usr/local/bin/
RUN chmod +x /usr/local/bin/gas-preprocessor.pl

# Update min macOS version for arm64
ENV OSX_VERSION_MIN="$MACOSX_ARM64_DEPLOYMENT_TARGET" \
	MACOSX_DEPLOYMENT_TARGET="$MACOSX_ARM64_DEPLOYMENT_TARGET"

# Install macOS dependencies required to build ffmpeg
ARG FFMPEG_DEPS
# hadolint ignore=SC2086
RUN --mount=type=cache,id=macports-arm64,target=/opt/osxcross/macports/cache \
	osxcross-macports install --arm64 $FFMPEG_DEPS

ARG FFMEG_OUT_DIR
RUN ./ffmpeg-build-macos.sh aarch64 "$MACOSX_SDK" "$FFMEG_OUT_DIR"

# ---
FROM scratch as ffmpeg-${FFMPEG_VERSION}

ARG FFMEG_OUT_DIR

COPY --from=x86_64 $FFMEG_OUT_DIR /ffmpeg/x86_64
COPY --from=aarch64 $FFMEG_OUT_DIR /ffmpeg/aarch64
