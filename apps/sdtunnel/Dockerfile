# syntax=docker/dockerfile:1.3-labs
FROM rust:latest as build

# Set working directory for build container
WORKDIR /app

# Cache the Rust build between Docker builds
RUN --mount=type=cache,target=/usr/local/cargo/registry
RUN --mount=type=cache,target=/app/target

# Copy prerequisites and install dependencies
# TODO: Make this work - it's difficult cause the Cargo.lock is outside the Docker build context
# RUN mkdir src && echo "fn main(){}" > src/main.rs
# COPY Cargo.toml ../../Cargo.lock /app/
# RUN mkdir .cargo && cargo vendor > .cargo/config

# Copy in code and build it
COPY . /app
RUN cargo build --release

# Create minimal non-root production container
FROM gcr.io/distroless/cc:nonroot

# Expose ports
EXPOSE 9000

# Copy in binary and set it as startup command
COPY --from=build /app/target/release/tunnel /
CMD ["/tunnel"]