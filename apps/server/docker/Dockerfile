ARG REPO_REF=main
ARG DEBIAN_FRONTEND=noninteractive

#--

FROM debian:bookworm as base

ARG DEBIAN_FRONTEND

ADD https://gist.githubusercontent.com/HeavenVolkoff/ff7b77b9087f956b8df944772e93c071/raw \
	/etc/apt/apt.conf.d/99docker-apt-config

RUN rm -f /etc/apt/apt.conf.d/docker-clean; \
	echo 'Binary::apt::APT::Keep-Downloaded-Packages "true";' > /etc/apt/apt.conf.d/keep-cache

RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt \
	apt-get update && apt-get upgrade

#--

FROM base as build

ARG DEBIAN_FRONTEND

RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt \
	apt-get install build-essential git sudo unzip wget

RUN wget -qO- https://get.pnpm.io/install.sh | env SHELL=bash PNPM_HOME=/usr/share/pnpm sh -

ENV PNPM_HOME="/usr/share/pnpm" \
	PATH="/usr/share/pnpm:$PATH"

RUN pnpm env use --global latest

RUN wget -qO- https://sh.rustup.rs | sh -s -- -yq --profile minimal

ENV PATH="/root/.cargo/bin:$PATH"

WORKDIR /srv

RUN git init spacedrive
RUN git -C spacedrive remote add origin https://github.com/spacedriveapp/spacedrive.git
RUN git -C spacedrive fetch --depth=1 origin "$REPO_REF"
RUN git -C spacedrive config advice.detachedHead false
RUN git -C spacedrive checkout FETCH_HEAD

WORKDIR /srv/spacedrive

RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt \
	env CI=true .github/scripts/setup-system.sh

RUN --mount=type=cache,target=/root/.cache/prisma/binaries/cli/ \
	pnpm prep

RUN cargo build --release -p server

#--

FROM base

ENV TZ=UTC \
	PUID=1000 \
	PGID=1000 \
	LANG=C.UTF-8 \
	LC_ALL=C.UTF-8 \
	TMPDIR=/tmp \
	LANGUAGE=en \
	DATA_DIR=/data

# Note: This needs to happen before the apt call to avoid locking issues with the previous step
COPY --from=build /srv/spacedrive/target/release/server /usr/bin/

RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt \
	apt-get install \
		libavdevice59 libpostproc56 libswscale6 libswresample4 libavformat59 libavutil57 libavfilter8 \
		libavcodec59 libheif1

COPY --chmod=755 entrypoint.sh /usr/bin/

# Expose webserver
EXPOSE 8080

# Create the data directory to store the database
VOLUME [ "/data" ]

# Run the CLI when the container is started
ENTRYPOINT [ "server" ]

LABEL org.opencontainers.image.title="Spacedrive Server" \
	org.opencontainers.image.source="https://github.com/spacedriveapp/spacedrive"
