ARG FAKE_DEPS="gettext-runtime libiconv ncurses" \
	FFMPEG_DEPS="brotli bzip2 dav1d fribidi libde265 libjxl libopus libpng libvorbis libvpx-devel \
	openjpeg soxr xz zimg" \
	HARFBUZZ_VERSION=7.3.0 \
	FREETYPE_VERSION=VER-2-13-1 \
	LIBASS_VERSION=0.17.1 \
	ZVBI_VERSION=0.2.41 \
	LIBWEBP_VERSION=1.3.1 \
	FFMPEG_VERSION=6.0 \
	LIBHEIF_VERSION=1.16.2

FROM vvaconcellos/osxcross:12.3-564e2b9-8 as base

SHELL ["/bin/bash", "-eux", "-o", "pipefail", "-c"]

WORKDIR /srv

ARG HARFBUZZ_VERSION
ADD https://github.com/harfbuzz/harfbuzz/releases/download/${HARFBUZZ_VERSION}/harfbuzz-${HARFBUZZ_VERSION}.tar.xz ./
RUN tar -xf "harfbuzz-${HARFBUZZ_VERSION}.tar.xz" && rm "harfbuzz-${HARFBUZZ_VERSION}.tar.xz" \
	&& \
	mv "/srv/harfbuzz-${HARFBUZZ_VERSION}" /srv/harfbuzz

ARG FREETYPE_VERSION
ADD https://gitlab.freedesktop.org/freetype/freetype/-/archive/${FREETYPE_VERSION}/freetype-${FREETYPE_VERSION}.tar.gz ./
RUN tar -xf "freetype-${FREETYPE_VERSION}.tar.gz" && rm "freetype-${FREETYPE_VERSION}.tar.gz" \
	&& \
	mv "/srv/freetype-${FREETYPE_VERSION}" /srv/freetype

ARG LIBASS_VERSION
ADD https://github.com/libass/libass/releases/download/${LIBASS_VERSION}/libass-${LIBASS_VERSION}.tar.xz ./
RUN tar -xf "libass-${LIBASS_VERSION}.tar.xz" && rm "libass-${LIBASS_VERSION}.tar.xz" \
	&& \
	mv "/srv/libass-${LIBASS_VERSION}" /srv/libass

ARG LIBWEBP_VERSION
ADD https://github.com/webmproject/libwebp/archive/refs/tags/v${LIBWEBP_VERSION}.tar.gz ./
RUN tar -xf "v${LIBWEBP_VERSION}.tar.gz" && rm "v${LIBWEBP_VERSION}.tar.gz" \
	&& \
	mv "/srv/libwebp-${LIBWEBP_VERSION}" /srv/libwebp

ARG ZVBI_VERSION
ADD https://github.com/zapping-vbi/zvbi/archive/refs/tags/v${ZVBI_VERSION}.tar.gz ./
RUN tar -xf "v${ZVBI_VERSION}.tar.gz" && rm "v${ZVBI_VERSION}.tar.gz" \
	&& \
	mv "/srv/zvbi-${ZVBI_VERSION}" /srv/zvbi \
	&& \
	cd /srv/zvbi \
	&& \
	for patch in \
	'https://github.com/videolan/vlc/raw/f7bb59d9f51cc10b25ff86d34a3eff744e60c46e/contrib/src/zvbi/zvbi-ssize_max.patch' \
	'https://github.com/videolan/vlc/raw/f7bb59d9f51cc10b25ff86d34a3eff744e60c46e/contrib/src/zvbi/zvbi-ioctl.patch' \
	'https://github.com/videolan/vlc/raw/f7bb59d9f51cc10b25ff86d34a3eff744e60c46e/contrib/src/zvbi/zvbi-fix-clang-support.patch' \
	; do curl -LSs "$patch" | patch -p1; done

ARG LIBHEIF_VERSION
ADD https://github.com/strukturag/libheif/releases/download/v${LIBHEIF_VERSION}/libheif-${LIBHEIF_VERSION}.tar.gz ./
RUN tar -xf "libheif-${LIBHEIF_VERSION}.tar.gz" && rm "libheif-${LIBHEIF_VERSION}.tar.gz" \
	&& \
	mv "/srv/libheif-${LIBHEIF_VERSION}" /srv/libheif

ARG FFMPEG_VERSION
ADD "https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VERSION}.tar.xz" ./
RUN tar -xf "ffmpeg-${FFMPEG_VERSION}.tar.xz" && rm "ffmpeg-${FFMPEG_VERSION}.tar.xz" \
	&& \
	mv "/srv/ffmpeg-${FFMPEG_VERSION}" /srv/ffmpeg

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
