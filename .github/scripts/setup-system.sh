#!/bin/bash

echo "Setting up your system for Spacedrive development!"

which cargo &> /dev/null
if [ $? -eq 1 ]; then
        echo "Rust was not detected on your system. Ensure the 'rustc' and 'cargo' binaries are in your \$PATH."
        exit 1
fi

which pnpm &> /dev/null
if [ $? -eq 1 ]; then
        echo "PNPM was not detected on your system. Ensure the 'pnpm' command is in your \$PATH. You are **not** able to use Yarn or NPM."
        exit 1
fi

if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        if which apt-get &> /dev/null; then
                echo "Detected 'apt' based distro!"
                DEBIAN_TAURI_DEPS="libwebkit2gtk-4.0-dev build-essential curl wget libssl-dev libgtk-3-dev libappindicator3-dev librsvg2-dev" # Tauri dependencies
                DEBIAN_FFMPEG_DEPS="libavcodec-dev libavdevice-dev libavfilter-dev libavformat-dev libavresample-dev libavutil-dev libswscale-dev libswresample-dev ffmpeg" # FFMPEG dependencies
                DEBIAN_BINDGEN_DEPS="pkg-config clang" # Bindgen dependencies - it's used by a dependency of Spacedrive

                sudo apt-get -y update
                sudo apt-get -y install $DEBIAN_TAURI_DEPS $DEBIAN_FFMPEG_DEPS $DEBIAN_BINDGEN_DEPS
        elif which pacman &> /dev/null; then
                echo "Detected 'pacman' based distro!"
                 sudo pacman -S --needed webkit2gtk base-devel curl wget openssl appmenu-gtk-module gtk3 libappindicator-gtk3 librsvg libvips

                ARCH_TAURI_DEPS="libwebkit2gtk-4.0-dev build-essential curl wget libssl-dev libgtk-3-dev libappindicator3-dev librsvg2-dev" # Tauri dependencies
                ARCH_FFMPEG_DEPS="" # FFMPEG dependencies # TODO
                ARCH_BINDGEN_DEPS="clang" # Bindgen dependencies - it's used by a dependency of Spacedrive

                sudo pacman -Syu
                sudo pacman -S --needed $ARCH_TAURI_DEPS $ARCH_FFMPEG_DEPS $ARCH_BINDGEN_DEPS

                # TODO: Remove warning
                echo "The FFMPEG dependencies are not yet included in this script for your Linux Distro. Please install them manually."
                echo "It would also be greatly appreciated if you could ping @oscartbeaumont in the Discord or GitHub so that you can help me work these out and update the script for everyone."
                exit 1
        elif which dnf &> /dev/null; then
                echo "Detected 'dnf' based distro!"
                FEDORA_TAURI_DEPS="webkit2gtk3-devel.x86_64 openssl-devel curl wget libappindicator-gtk3 librsvg2-devel" # Tauri dependencies
                FEDORA_FFMPEG_DEPS="ffmpeg ffmpeg-devel" # FFMPEG dependencies
                FEDORA_BINDGEN_DEPS="clang" # Bindgen dependencies - it's used by a dependency of Spacedrive

                sudo dnf check-update
                sudo dnf install $FEDORA_TAURI_DEPS $FEDORA_FFMPEG_DEPS $FEDORA_BINDGEN_DEPS
                sudo dnf group install "C Development Tools and Libraries"

                # TODO: Remove warning
                echo "It would also be greatly appreciated if you could ping @oscartbeaumont in the Discord or GitHub so that you can help me work these out and update the script for everyone."
                exit 1
        else
                echo "Your Linux distro '$(lsb_release -s -d)' is not supported by this script. We would welcome a PR or some help adding your OS to this script. https://github.com/spacedriveapp/spacedrive/issues"
                exit 1
        fi

        
elif [[ "$OSTYPE" == "darwin"* ]]; then
        brew install ffmpeg
else
        echo "Your OS '$OSTYPE' is not supported by this script. We would welcome a PR or some help adding your OS to this script. https://github.com/spacedriveapp/spacedrive/issues"
        exit 1
fi
