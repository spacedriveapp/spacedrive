# Spacedrive Server Docker Image
# Multi-stage build for minimal production image

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
# Bun builder (for web assets)
#--
FROM oven/bun:1.3-slim AS node-builder

WORKDIR /build

# Copy all source code first
COPY package.json bun.lock tsconfig.json ./
COPY packages ./packages
COPY apps/web ./apps/web

# Create placeholder for scripts workspace (referenced in package.json)
RUN mkdir -p scripts && echo '{"name":"scripts","private":true}' > scripts/package.json

# Install dependencies
# Note: Not using --frozen-lockfile because we add a placeholder scripts package
# Allow partial failures - some workspace packages may not install perfectly in Docker but web build will work
RUN bun install; exit 0

# Build ts-client package (required by web app)
WORKDIR /build/packages/ts-client
RUN bun run build

# Build web assets
WORKDIR /build/apps/web
RUN bun run build

#--
# Rust builder
#--
FROM base AS rust-builder

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

WORKDIR /build

# Copy workspace configuration
COPY Cargo.toml Cargo.lock ./
COPY .cargo ./.cargo

# Copy source code
COPY core ./core
COPY crates ./crates
COPY apps/server ./apps/server

# Copy built web assets from node-builder
COPY --from=node-builder /build/apps/web/dist ./apps/web/dist

# Build server with embedded assets
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
COPY --from=rust-builder /usr/local/bin/sd-server /usr/bin/sd-server

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
