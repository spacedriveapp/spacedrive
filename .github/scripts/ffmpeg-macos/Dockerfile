ARG FAKE_DEPS="python311 perl5.34 gdk-pixbuf2 xorg-libsm xorg-libX11" \
	FFMPEG_DEPS="aom bzip2 fontconfig freetype fribidi lame libgsm libheif libogg libopus libpng \
	libtheora libvidstab libvorbis libvpx-devel lzo2 openjpeg rav1e soxr svt-av1 twolame webp x264 \
	x265 XviD xz zimg zlib" \
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
RUN ./ffmpeg-build-macos.sh x86_64 "$MACOSX_SDK"

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
RUN ./ffmpeg-build-macos.sh aarch64 "$MACOSX_SDK"

# ---
FROM scratch

COPY --from=x86_64 /FFMpeg.framework /ffmpeg/x86_64/FFMpeg.framework
COPY --from=aarch64 /FFMpeg.framework /ffmpeg/aarch64/FFMpeg.framework
