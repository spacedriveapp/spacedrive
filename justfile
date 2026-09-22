# Spacedrive development commands

# Install JS dependencies and generate the cargo config
setup:
    bun install
    cargo xtask setup

# Same, plus the prebuilt codec bundle (FFmpeg, libheif, Pdfium)
setup-native-deps:
    bun install
    cargo xtask setup --native-deps

# Run the daemon (default dev workflow: just dev-daemon + just dev-desktop)
dev-daemon *ARGS:
	cargo run --bin sd-daemon {{ARGS}}

# Run the desktop app in dev mode
dev-desktop: build-apps
    cd apps/tauri && bun run tauri:dev:no-watch

# Run the desktop app in dev mode, rebuilding on Rust changes
dev-desktop-watch: build-apps
    cd apps/tauri && bun run tauri:dev

# Build the apps the desktop Apps menu launches, beside the desktop binary.
# sd-native is not a default member and can lag behind core, so a failed
# build warns instead of stopping the desktop app.
build-apps:
    cargo build -p sd-native || echo "warning: sd-native did not build; Apps > Photos runs the previous build, if there is one"

# Run the docs site in dev mode (Fumadocs, standalone install in docs/)
dev-docs:
    cd docs && bun install && bun run dev

# Build the docs site
build-docs:
    cd docs && bun install && bun run build

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

# Link SpaceUI packages for local development.
spaceui-link:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ ! -d ../spaceui/packages ]; then
        echo "Error: ../spaceui not found. Clone it adjacent to this repo:"
        echo "  git clone https://github.com/spacedriveapp/spaceui ../spaceui"
        exit 1
    fi
    cd ../spaceui
    bun install && bun run build --filter='@spacedrive/primitives' --filter='@spacedrive/ai' --filter='@spacedrive/forms' --filter='@spacedrive/explorer' --filter='@spacedrive/tokens'
    for pkg in primitives ai forms explorer tokens; do
        cd packages/$pkg && bun link && cd ../..
    done
    cd "{{justfile_directory()}}"
    bun link @spacedrive/primitives @spacedrive/ai @spacedrive/forms @spacedrive/explorer @spacedrive/tokens
    echo "SpaceUI packages linked successfully."

# Unlink SpaceUI packages and restore npm versions.
spaceui-unlink:
    cd packages/interface && bun unlink @spacedrive/primitives @spacedrive/ai @spacedrive/forms @spacedrive/explorer @spacedrive/tokens && bun install

# Run the CLI
cli *ARGS:
    cargo run --bin sd-cli -- {{ARGS}}
