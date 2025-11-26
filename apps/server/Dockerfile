# Spacedrive Server Docker Image
# Single-stage build for RPC-only server (no web UI)

FROM debian:bookworm-slim AS builder

# Install build dependencies
RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt \
	apt-get update && apt-get install -y \
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

# Build server (RPC only, no assets)
RUN --mount=type=cache,target=/root/.cargo/registry \
	--mount=type=cache,target=/root/.cargo/git \
	--mount=type=cache,target=/build/target \
	cargo build --release -p sd-server && \
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
