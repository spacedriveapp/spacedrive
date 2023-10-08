ARG REPO=spacedriveapp/spacedrive
ARG REPO_REF=main
ARG DEBIAN_FRONTEND=noninteractive

#--

FROM debian:bookworm as base

ARG DEBIAN_FRONTEND

COPY https://gist.githubusercontent.com/HeavenVolkoff/ff7b77b9087f956b8df944772e93c071/raw \
	/etc/apt/apt.conf.d/99docker-apt-config

USER spacedrive
HEALTHCHECK NONE

# trunk-ignore(terrascan/AC_DOCKER_0002)
RUN rm -f /etc/apt/apt.conf.d/docker-clean; \
	echo 'Binary::apt::APT::Keep-Downloaded-Packages "true";' > /etc/apt/apt.conf.d/keep-cache

# RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt \
# 	apt-get update && apt-get upgrade


#--

FROM base as build-base

ARG DEBIAN_FRONTEND

RUN apt-get update && apt-get install -y --no-install-recommends \
	build-essential=12.9 \
	git=1:2.39.2-1.1 \
	unzip=6.0-28 \
	wget=1.21.3-1


# trunk-ignore(hadolint/DL4006)
RUN ["/bin/bash", "-c", "set -o pipefail && wget -qO- https://get.pnpm.io/install.sh | env SHELL=bash PNPM_HOME=/usr/share/pnpm sh -"]

ENV PNPM_HOME="/usr/share/pnpm" \
	PATH="/usr/share/pnpm:$PATH"

RUN pnpm env use --global latest

WORKDIR /srv

ARG REPO REPO_REF
RUN git init spacedrive &&\
	git -C spacedrive remote add origin "https://github.com/${REPO}.git" &&\
	git -C spacedrive fetch --depth=1 origin "$REPO_REF" &&\
	git -C spacedrive config advice.detachedHead false &&\
	git -C spacedrive checkout FETCH_HEAD

WORKDIR /srv/spacedrive

#--

FROM build-base as web

# Run pnpm install with docker cache
RUN --mount=type=cache,target=/root/.local/share/pnpm/store --mount=type=cache,target=/root/.cache/pnpm/metadata \
	pnpm install --frozen-lockfile

ENV NODE_OPTIONS="--max-old-space-size=4096"

RUN pnpm web build

#--

FROM build-base as server

# trunk-ignore(hadolint/DL4006)
RUN ["/bin/bash", "-c", "set -o pipefail && wget -qO- https://sh.rustup.rs | sh -s -- -yq --profile minimal"]

ENV PATH="/root/.cargo/bin:$PATH" \
	WEBUI_PORT=8080

RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt \
	env CI=true ./scripts/setup.sh

RUN --mount=type=cache,target=/root/.cache/prisma/binaries/cli/ \
	pnpm prep

COPY --from=web /srv/spacedrive/apps/web/dist /srv/spacedrive/apps/web/dist

RUN cargo build --features assets --release -p sd-server

#--

FROM base

ENV TZ=UTC \
	PUID=1000 \
	PGID=1000 \
	LANG=C.UTF-8 \
	LC_ALL=C.UTF-8 \
	TMPDIR=/tmp \
	LANGUAGE=en \
	DATA_DIR=/data \
	WEBUI_PORT=8080

# Note: This needs to happen before the apt call to avoid locking issues with the previous step
COPY --from=server /srv/spacedrive/target/release/sd-server /usr/bin/

RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt \
RUN apt-get update && apt-get install --no-install-recommends -y \
	libavdevice59=7:5.1.3-1 \
	libpostproc56=7:5.1.3-1 \
	libswscale6=7:5.1.3-1 \
	libswresample4=7:5.1.3-1 \
	libavformat59=7:5.1.3-1 \
	libavutil57=7:5.1.3-1 \
	libavfilter8=7:5.1.3-1 \
	libavcodec59=7:5.1.3-1 \
	libheif1=1.15.1-1

COPY --chmod=755 entrypoint.sh /usr/bin/

# Expose webserver
EXPOSE $WEBUI_PORT

# Create the data directory to store the database
VOLUME [ "/data" ]

# Run the CLI when the container is started
ENTRYPOINT [ "sd-server" ]

HEALTHCHECK --interval=5m --timeout=5s \
	CMD curl -f http://localhost:$WEBUI_PORT/ || exit 1

LABEL org.opencontainers.image.title="Spacedrive Server" \
	org.opencontainers.image.source="https://github.com/spacedriveapp/spacedrive"