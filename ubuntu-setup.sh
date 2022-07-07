#!/bin/bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs > rustup.sh
chmod +x rustup.sh
sudo ./rustup.sh -y
rm -f rustup.sh
sudo npm install -y -g npm@latest pnpm
# this is the distro specific part
DEBIAN_TAURI_DEPS="libwebkit2gtk-4.0-dev build-essential curl wget libssl-dev libgtk-3-dev libappindicator3-dev librsvg2-dev libdbus-1-dev libavutil-dev" # Tauri dependencies
DEBIAN_FFMPEG_DEPS="libavcodec-dev libavdevice-dev libavfilter-dev libavformat-dev libavresample-dev libavutil-dev libswscale-dev libswresample-dev ffmpeg" # FFMPEG dependencies
DEBIAN_BINDGEN_DEPS="pkg-config clang" # Bindgen dependencies - it's used by a dependency of Spacedrive
sudo apt-get -y update
sudo apt-get -y install ${SPACEDRIVE_CUSTOM_APT_FLAGS:-} $DEBIAN_TAURI_DEPS $DEBIAN_FFMPEG_DEPS $DEBIAN_BINDGEN_DEPS
while read -r env; do export "$env"; done

pnpm setup
pnpm i
cargo install -y tauri-cli
pnpm prep