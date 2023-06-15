ARG FFMPEG_VERSION=6.0

FROM ghcr.io/btbn/ffmpeg-builds/base-win64:latest@sha256:69e7ee385c41ae4580161fbd1cf14e1d199a81c39e398e8421225d86d8f2d3f3 AS base

ENV TARGET=win64

FROM base AS layer-10-mingw

RUN --mount=src=scripts.d/10-mingw.sh,dst=/stage.sh run_stage /stage.sh

FROM base AS layer-10-mingw-std-threads

RUN --mount=src=scripts.d/10-mingw-std-threads.sh,dst=/stage.sh run_stage /stage.sh

FROM base AS layer-10

COPY --from=layer-10-mingw /opt/mingw/. /

COPY --from=layer-10-mingw /opt/mingw/. /opt/mingw

COPY --from=layer-10-mingw-std-threads $FFBUILD_PREFIX/. $FFBUILD_PREFIX

FROM layer-10 AS layer-20-zlib

RUN --mount=src=scripts.d/20-zlib.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-10 AS layer-20-bzip2

RUN --mount=src=scripts.d/20-bzip2.sh,dst=/stage.sh --mount=src=patches/bzip2,dst=/patches run_stage /stage.sh

FROM layer-10 AS layer-20

COPY --from=layer-20-zlib $FFBUILD_PREFIX/. $FFBUILD_PREFIX

COPY --from=layer-20-bzip2 $FFBUILD_PREFIX/. $FFBUILD_PREFIX

FROM layer-20 AS layer-25-libogg

RUN --mount=src=scripts.d/25-libogg.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-20 AS layer-25-xz

RUN --mount=src=scripts.d/25-xz.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-20 AS layer-25

COPY --from=layer-25-libogg $FFBUILD_PREFIX/. $FFBUILD_PREFIX

COPY --from=layer-25-xz $FFBUILD_PREFIX/. $FFBUILD_PREFIX

FROM layer-25 AS layer-45-brotli

RUN --mount=src=scripts.d/45-brotli.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-25 AS layer-45-libvorbis

RUN --mount=src=scripts.d/45-libvorbis.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-25 AS layer-45-opencl

RUN --mount=src=scripts.d/45-opencl.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-25 AS layer-45-vmaf

RUN --mount=src=scripts.d/45-vmaf.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-25 AS layer-45

COPY --from=layer-45-brotli $FFBUILD_PREFIX/. $FFBUILD_PREFIX

COPY --from=layer-45-libvorbis $FFBUILD_PREFIX/. $FFBUILD_PREFIX

COPY --from=layer-45-opencl $FFBUILD_PREFIX/. $FFBUILD_PREFIX

COPY --from=layer-45-vmaf $FFBUILD_PREFIX/. $FFBUILD_PREFIX

FROM layer-45 AS layer-50-amf

RUN --mount=src=scripts.d/50-amf.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-45 AS layer-50-dav1d

RUN --mount=src=scripts.d/50-dav1d.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-45 AS layer-50-ffnvcodec

RUN --mount=src=scripts.d/50-ffnvcodec.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-45 AS layer-50-kvazaar

RUN --mount=src=scripts.d/50-kvazaar.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-45 AS layer-50-libde265

RUN --mount=src=scripts.d/50-libde265.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-45 AS layer-50-libjxl

RUN --mount=src=scripts.d/50-libjxl.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-45 AS layer-50-libmp3lame

RUN --mount=src=scripts.d/50-libmp3lame.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-45 AS layer-50-libopus

RUN --mount=src=scripts.d/50-libopus.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-45 AS layer-50-libtheora

RUN --mount=src=scripts.d/50-libtheora.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-45 AS layer-50-libvpx

RUN --mount=src=scripts.d/50-libvpx.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-45 AS layer-50-libwebp

RUN --mount=src=scripts.d/50-libwebp.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-45 AS layer-50-onevpl

RUN --mount=src=scripts.d/50-onevpl.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-45 AS layer-50-openal

RUN --mount=src=scripts.d/50-openal.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-45 AS layer-50-openjpeg

RUN --mount=src=scripts.d/50-openjpeg.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-45 AS layer-50-rav1e

RUN --mount=src=scripts.d/50-rav1e.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-45 AS layer-50-soxr

RUN --mount=src=scripts.d/50-soxr.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-45 AS layer-50-twolame

RUN --mount=src=scripts.d/50-twolame.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-45 AS layer-50-vulkan

RUN --mount=src=scripts.d/50-vulkan/45-vulkan.sh,dst=/stage.sh run_stage /stage.sh

RUN --mount=src=scripts.d/50-vulkan/50-shaderc.sh,dst=/stage.sh run_stage /stage.sh

RUN --mount=src=scripts.d/50-vulkan/55-spirv-cross.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-45 AS layer-50-x264

RUN --mount=src=scripts.d/50-x264.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-45 AS layer-50-x265

RUN --mount=src=scripts.d/50-x265.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-45 AS layer-50-xvid

RUN --mount=src=scripts.d/50-xvid.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-45 AS layer-50-zimg

RUN --mount=src=scripts.d/50-zimg.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-45 AS layer-50

COPY --from=layer-50-amf $FFBUILD_PREFIX/. $FFBUILD_PREFIX

COPY --from=layer-50-dav1d $FFBUILD_PREFIX/. $FFBUILD_PREFIX
COPY --from=layer-50-dav1d /opt/dlls /opt/dlls

COPY --from=layer-50-ffnvcodec $FFBUILD_PREFIX/. $FFBUILD_PREFIX

COPY --from=layer-50-kvazaar $FFBUILD_PREFIX/. $FFBUILD_PREFIX

COPY --from=layer-50-libde265 $FFBUILD_PREFIX/. $FFBUILD_PREFIX

COPY --from=layer-50-libjxl $FFBUILD_PREFIX/. $FFBUILD_PREFIX

COPY --from=layer-50-libmp3lame $FFBUILD_PREFIX/. $FFBUILD_PREFIX

COPY --from=layer-50-libopus $FFBUILD_PREFIX/. $FFBUILD_PREFIX

COPY --from=layer-50-libtheora $FFBUILD_PREFIX/. $FFBUILD_PREFIX

COPY --from=layer-50-libvpx $FFBUILD_PREFIX/. $FFBUILD_PREFIX

COPY --from=layer-50-libwebp $FFBUILD_PREFIX/. $FFBUILD_PREFIX

COPY --from=layer-50-onevpl $FFBUILD_PREFIX/. $FFBUILD_PREFIX

COPY --from=layer-50-openal $FFBUILD_PREFIX/. $FFBUILD_PREFIX

COPY --from=layer-50-openjpeg $FFBUILD_PREFIX/. $FFBUILD_PREFIX

COPY --from=layer-50-rav1e $FFBUILD_PREFIX/. $FFBUILD_PREFIX
COPY --from=layer-50-rav1e /opt/dlls /opt/dlls

COPY --from=layer-50-soxr $FFBUILD_PREFIX/. $FFBUILD_PREFIX

COPY --from=layer-50-twolame $FFBUILD_PREFIX/. $FFBUILD_PREFIX

COPY --from=layer-50-vulkan $FFBUILD_PREFIX/. $FFBUILD_PREFIX

COPY --from=layer-50-x264 $FFBUILD_PREFIX/. $FFBUILD_PREFIX

COPY --from=layer-50-x265 $FFBUILD_PREFIX/. $FFBUILD_PREFIX
COPY --from=layer-50-x265 /opt/dlls /opt/dlls

COPY --from=layer-50-xvid $FFBUILD_PREFIX/. $FFBUILD_PREFIX

COPY --from=layer-50-zimg $FFBUILD_PREFIX/. $FFBUILD_PREFIX

FROM layer-50 AS layer-99-libheif

RUN --mount=src=scripts.d/99-libheif.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-50 AS layer-99-ffmpeg

ARG FFMPEG_VERSION
ENV FFMPEG_VERSION="$FFMPEG_VERSION"

RUN --mount=src=scripts.d/99-ffmpeg.sh,dst=/stage.sh run_stage /stage.sh

FROM layer-50 AS layer-99

COPY --from=layer-50 /opt/dlls /opt/dlls

COPY --from=layer-99-libheif /opt/dlls /opt/dlls

COPY --from=layer-99-ffmpeg /opt/dlls /opt/dlls

RUN rm -rf /opt/dlls/share /opt/dlls/lib/pkgconfig /opt/dlls/lib/cmake

RUN cd /opt/dlls/bin \
	&& find . -name '*.lib' -exec install -Dt ../lib/ -m a-rwx,u+rw,g+r,o+r {} + \
	&& find . -name '*.lib' -delete \
	&& find . -name '*.dll' -exec x86_64-w64-mingw32-strip -s {} \;

RUN find /opt/dlls -type d -delete || true

FROM scratch

COPY --from=layer-99 /opt/dlls /dlls
