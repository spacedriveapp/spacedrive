# Spacedrive development commands

# Install JS dependencies and set up native deps + cargo config
setup:
    bun install
    cargo xtask setup

# Run the daemon (default dev workflow: just dev-daemon + just dev-desktop)
dev-daemon *ARGS:
	cargo run --features ffmpeg,heif --bin sd-daemon {{ARGS}}

# Run the desktop app in dev mode
dev-desktop:
    cd apps/tauri && bun run tauri:dev

# Run the mobile app in dev mode
dev-mobile:
	cd apps/mobile && bun run start

# Run the mobile app on iOS
dev-mobile-ios:
	cd apps/mobile && bun run ios

# Run the mobile app on Android
dev-mobile-android:
	cd apps/mobile && bun run android

# Build the native mobile core
build-mobile:
	cargo xtask build-mobile

# Run the headless server (web UI, no desktop app)
dev-server *ARGS:
    cargo run --bin sd-server {{ARGS}}

# Run all workspace tests
test:
    cargo test --workspace

# Build everything (default members)
build:
    cargo build

# Build in release mode
build-release:
    cargo build --release

# Format and lint
check:
    cargo fmt --check
    cargo clippy --workspace

# Format code
fmt:
    cargo fmt

# Run the CLI
cli *ARGS:
    cargo run --bin sd-cli -- {{ARGS}}
