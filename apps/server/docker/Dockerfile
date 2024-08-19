ARG REPO=spacedriveapp/spacedrive
ARG REPO_REF=main

#--

FROM debian:bookworm AS base

ADD --chmod=644 --checksum=sha256:8bea540b2cd1a47c94555e746c75fd41a42847a46d8c8c36c7ab6dd9c8526ab4 \
	https://gist.githubusercontent.com/HeavenVolkoff/ff7b77b9087f956b8df944772e93c071/raw \
	/etc/apt/apt.conf.d/99docker-apt-config

RUN echo 'debconf debconf/frontend select Noninteractive' | debconf-set-selections

RUN rm -f /etc/apt/apt.conf.d/docker-clean

RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt \
	apt-get update && apt-get upgrade

#--

FROM base AS build-base

RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt \
	apt-get install build-essential git sudo unzip wget curl python3 python-is-python3

RUN wget -qO- https://get.pnpm.io/install.sh | env SHELL=bash PNPM_HOME=/usr/share/pnpm sh -

ENV PNPM_HOME="/usr/share/pnpm" \
	PATH="/usr/share/pnpm:$PATH"

RUN pnpm env use --global latest
RUN pnpm add -g pnpm@latest-9

WORKDIR /srv

ARG REPO REPO_REF
RUN git init spacedrive
RUN git -C spacedrive remote add origin "https://github.com/${REPO}.git"
RUN git -C spacedrive fetch --depth=1 origin "$REPO_REF"
RUN git -C spacedrive config advice.detachedHead false
RUN git -C spacedrive checkout FETCH_HEAD

WORKDIR /srv/spacedrive

#--

FROM build-base AS web

# Run pnpm install with docker cache
RUN --mount=type=cache,target=/root/.local/share/pnpm/store --mount=type=cache,target=/root/.cache/pnpm/metadata \
	pnpm install --frozen-lockfile

ENV NODE_OPTIONS="--max-old-space-size=4096"

RUN pnpm web build

#--

FROM build-base AS server

RUN --mount=type=cache,target=/root/.rustup/downloads \
	wget -qO- https://sh.rustup.rs | sh -s -- -yq --profile minimal

ENV PATH="/root/.cargo/bin:$PATH"

# From: https://github.com/rust-lang/rust-analyzer/blob/master/.github/workflows/release.yaml#L13-L21
ENV CARGO_NET_RETRY=10
ENV CARGO_INCREMENTAL=0
ENV RUSTUP_MAX_RETRIES=10

# Install mold (linker)
RUN curl -L# 'https://github.com/rui314/mold/releases/download/v2.4.0/mold-2.4.0-x86_64-linux.tar.gz' \
	| sudo tar -xzf- -C /usr/local

RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt \
	env CI=true ./scripts/setup.sh

RUN cd ./scripts; npm i --omit=dev && npm i -g prisma

RUN cargo --version

RUN --mount=type=cache,target=/root/.rustup/downloads \
	env PATH="$(npm prefix -g)/bin:$PATH" pnpm prep

COPY --from=web /srv/spacedrive/apps/web/dist /srv/spacedrive/apps/web/dist

RUN --mount=type=cache,target=/root/.rustup/downloads \
	cargo build --features assets --release -p sd-server

#--

# Debug just means it includes busybox, and we need its tools both for the entrypoint.sh script and during runtime
FROM gcr.io/distroless/base-debian12:debug

RUN [ "/busybox/ln", "-s", "/busybox/sh", "/bin/sh" ]
RUN ln -s /busybox/env /usr/bin/env

ENV TZ=UTC \
	PUID=1000 \
	PGID=1000 \
	LANG=C.UTF-8 \
	LC_ALL=C.UTF-8 \
	TMPDIR=/tmp \
	LANGUAGE=en \
	DATA_DIR=/data

COPY --from=server --chmod=755 /srv/spacedrive/target/release/sd-server /usr/bin/
COPY --from=server --chmod=755 /lib/x86_64-linux-gnu/libgcc_s.so.1 /usr/lib/
COPY --from=server --chmod=755 /srv/spacedrive/apps/.deps/lib /usr/lib/spacedrive

ADD --chmod=755 --checksum=sha256:a99beabea22571cfad4f77422e5d3ed922d9490232d94cb87cf32956766bc42a \
	https://github.com/kraj/uClibc/raw/v0.9.33.2/extra/scripts/getent /usr/bin/
ADD --chmod=755 --checksum=sha256:1d127c69218f2cd14964036f2b057c4b2652cda3996c6908605cc139192f66aa \
	https://github.com/spacedriveapp/native-deps/releases/download/yolo-2024-02-07/yolov8s.onnx /usr/share/spacedrive/models/yolov8s.onnx

COPY --chmod=755 entrypoint.sh /usr/bin/

# P2P config
ENV SD_DOCKER=true

# Expose webserver
EXPOSE 8080

# Expose P2P
EXPOSE 7373

# Create the data directory to store the database
VOLUME [ "/data" ]

# Run the CLI when the container is started
ENTRYPOINT [ "entrypoint.sh" ]

LABEL org.opencontainers.image.title="Spacedrive Server" \
	org.opencontainers.image.source="https://github.com/spacedriveapp/spacedrive"
