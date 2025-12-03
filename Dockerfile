# Multi-stage Dockerfile for Spacedrive CLI and Daemon
# Supports: x86_64 and aarch64 Linux

# ============================================================================
# Builder Stage - Compile Rust binaries
# ============================================================================
FROM rust:1.81-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
	build-essential \
	pkg-config \
	libssl-dev \
	git \
	&& rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /build

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY apps/ apps/
COPY core/ core/
COPY crates/ crates/
COPY xtask/ xtask/

# Copy dependencies (specta, opendal)
COPY specta/ specta/
COPY opendal/ opendal/

# Build release binaries
# Note: We only build CLI features, no FFmpeg or AI models needed
RUN cargo build --release --bin sd-cli --bin sd-daemon

# ============================================================================
# Runtime Stage - Minimal image with only runtime dependencies
# ============================================================================
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
	ca-certificates \
	libssl3 \
	&& rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 spacedrive

# Create data directory
RUN mkdir -p /data && chown spacedrive:spacedrive /data

# Copy binaries from builder
COPY --from=builder /build/target/release/sd-cli /usr/local/bin/sd-cli
COPY --from=builder /build/target/release/sd-daemon /usr/local/bin/sd-daemon

# Set permissions
RUN chmod +x /usr/local/bin/sd-cli /usr/local/bin/sd-daemon

# Switch to non-root user
USER spacedrive

# Set data directory as volume
VOLUME /data

# Expose any ports if needed (future: add when API is enabled)
# EXPOSE 8080

# Set environment variables
ENV SPACEDRIVE_DATA_DIR=/data

# Healthcheck
HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
	CMD sd-cli status || exit 1

# Default command: start daemon in foreground
CMD ["sd-cli", "--data-dir", "/data", "start", "--foreground"]
