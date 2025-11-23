# Spacedrive Server Docker Image
# Multi-stage build for minimal production image

ARG RUST_VERSION=1.83
ARG NODE_VERSION=22

#--
# Base image with common dependencies
#--
FROM debian:bookworm-slim AS base

# Configure apt for non-interactive use and caching
RUN echo 'debconf debconf/frontend select Noninteractive' | debconf-set-selections
RUN rm -f /etc/apt/apt.conf.d/docker-clean

RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt \
	apt-get update && apt-get upgrade -y

#--
# Build environment
#--
FROM base AS builder

# Install build dependencies
RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt \
	apt-get install -y \
	build-essential \
	curl \
	git \
	pkg-config \
	libssl-dev \
	ca-certificates

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
ENV PATH="/root/.cargo/bin:$PATH"

# Install Node.js and pnpm
RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt \
	curl -fsSL https://deb.nodesource.com/gpgkey/nodesource-repo.gpg.key | gpg --dearmor -o /etc/apt/keyrings/nodesource.gpg && \
	echo "deb [signed-by=/etc/apt/keyrings/nodesource.gpg] https://deb.nodesource.com/node_${NODE_VERSION}.x nodistro main" | tee /etc/apt/sources.list.d/nodesource.list && \
	apt-get update && \
	apt-get install -y nodejs
RUN npm install -g pnpm@latest

WORKDIR /build

# Copy workspace configuration
COPY Cargo.toml Cargo.lock ./
COPY .cargo ./.cargo

# Copy source code
COPY core ./core
COPY crates ./crates
COPY apps/server ./apps/server
COPY apps/web ./apps/web
COPY packages ./packages

# Build web assets first
WORKDIR /build/apps/web
RUN --mount=type=cache,target=/root/.local/share/pnpm/store \
	pnpm install --frozen-lockfile
RUN pnpm build

# Build server with embedded assets
WORKDIR /build
RUN --mount=type=cache,target=/root/.cargo/registry \
	--mount=type=cache,target=/root/.cargo/git \
	--mount=type=cache,target=/build/target \
	cargo build --release --features assets -p sd-server && \
	cp target/release/sd-server /usr/local/bin/sd-server

#--
# Runtime image
#--
FROM gcr.io/distroless/cc-debian12:nonroot

# Copy binary
COPY --from=builder /usr/local/bin/sd-server /usr/bin/sd-server

# Environment
ENV DATA_DIR=/data \
	PORT=8080 \
	RUST_LOG=info,sd_core=debug

# Expose HTTP port
EXPOSE 8080

# Expose P2P port
EXPOSE 7373

# Volume for persistent data
VOLUME ["/data"]

# Run as non-root user
USER nonroot:nonroot

# Start server
ENTRYPOINT ["/usr/bin/sd-server"]
CMD ["--data-dir", "/data"]
