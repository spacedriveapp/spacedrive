ARG REPO=spacedriveapp/spacedrive
ARG REPO_REF=main

#--

FROM debian:bookworm as base

ADD https://gist.githubusercontent.com/HeavenVolkoff/ff7b77b9087f956b8df944772e93c071/raw \
	/etc/apt/apt.conf.d/99docker-apt-config

RUN echo 'debconf debconf/frontend select Noninteractive' | debconf-set-selections

RUN rm -f /etc/apt/apt.conf.d/docker-clean

RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt \
	apt-get update && apt-get upgrade

#--

FROM base as build-base

RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt \
	apt-get install build-essential git sudo unzip wget

RUN wget -qO- https://get.pnpm.io/install.sh | env SHELL=bash PNPM_HOME=/usr/share/pnpm sh -

ENV PNPM_HOME="/usr/share/pnpm" \
	PATH="/usr/share/pnpm:$PATH"

RUN pnpm env use --global latest

WORKDIR /srv

ARG REPO REPO_REF
RUN git init spacedrive
RUN git -C spacedrive remote add origin "https://github.com/${REPO}.git"
RUN git -C spacedrive fetch --depth=1 origin "$REPO_REF"
RUN git -C spacedrive config advice.detachedHead false
RUN git -C spacedrive checkout FETCH_HEAD

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

RUN wget -qO- https://sh.rustup.rs | sh -s -- -yq --profile minimal

ENV PATH="/root/.cargo/bin:$PATH"

RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt \
	env CI=true ./scripts/setup.sh

RUN cd ./scripts; npm i --production

RUN --mount=type=cache,target=/root/.cache/prisma/binaries/cli/ \
	pnpm prep

COPY --from=web /srv/spacedrive/apps/web/dist /srv/spacedrive/apps/web/dist

RUN cargo build --features assets --release -p sd-server

#--

# Debug just means it includes busybox, and we need its tools both for the entrypoint.sh script and during runtime
FROM gcr.io/distroless/base-debian12:debug

ENV TZ=UTC \
	PUID=1000 \
	PGID=1000 \
	LANG=C.UTF-8 \
	LC_ALL=C.UTF-8 \
	TMPDIR=/tmp \
	LANGUAGE=en \
	DATA_DIR=/data

COPY --from=server /srv/spacedrive/target/release/sd-server /usr/bin/
COPY --from=server /lib/x86_64-linux-gnu/libgcc_s.so.1 /usr/lib/
COPY --from=server /srv/spacedrive/apps/.deps/lib /usr/lib/spacedrive

COPY --chmod=755 entrypoint.sh /usr/bin/

# Expose webserver
EXPOSE 8080

# Create the data directory to store the database
VOLUME [ "/data" ]

# Run the CLI when the container is started
ENTRYPOINT [ "sd-server" ]

LABEL org.opencontainers.image.title="Spacedrive Server" \
	org.opencontainers.image.source="https://github.com/spacedriveapp/spacedrive"
