#!/bin/bash

set -e

script_failure() {
        echo "An error occurred while performing the task on line $1" >&2
        echo "Setup for Spacedrive development failed" >&2
}

trap 'script_failure $LINENO' ERR

echo "Setting up your system for Spacedrive development!"

if ! which cargo &> /dev/null; then
        echo "Rust was not detected on your system. Ensure the 'rustc' and 'cargo' binaries are in your \$PATH."
        exit 1
fi



if [ "$1" == "mobile" ]; then
        echo "Setting up for mobile development!"

         # IOS targets
        if [[ "$OSTYPE" == "darwin"* ]]; then
                echo "Installing IOS Rust targets..."

                if ! /usr/bin/xcodebuild -version; then
                        echo "Xcode is not installed! Ensure you have it installed!"
                        exit 1
                fi

                rustup target add aarch64-apple-ios
                rustup target add aarch64-apple-ios-sim
        fi

        # Android requires python
        if ! command -v python3 &> /dev/null
        then
                echo "Python3 could not be found. This is required for Android mobile development!"
                exit 1
        fi

        # Android targets
        echo "Installing Android Rust targets..."
        rustup target add armv7-linux-androideabi   # for arm
        rustup target add i686-linux-android        # for x86
        rustup target add aarch64-linux-android     # for arm64
        rustup target add x86_64-linux-android      # for x86_64
        rustup target add x86_64-unknown-linux-gnu  # for linux-x86-64
        rustup target add x86_64-apple-darwin       # for darwin x86_64 (if you have an Intel MacOS)
        rustup target add aarch64-apple-darwin      # for darwin arm64 (if you have a M1 MacOS)
        rustup target add x86_64-pc-windows-gnu     # for win32-x86-64-gnu
        rustup target add x86_64-pc-windows-msvc    # for win32-x86-64-msvc
fi

KNOWN_DISTRO="(Debian|Ubuntu|RedHat|CentOS|openSUSE|Arch|Fedora)"
DISTRO=$(awk -F= '$1=="ID" { print $2 ;}' /etc/os-release 2>/dev/null | grep -Eo $KNOWN_DISTRO  || grep -Eo $KNOWN_DISTRO /etc/issue 2>/dev/null || uname -s | grep -Eo $KNOWN_DISTRO  || grep -Eo $KNOWN_DISTRO /etc/issue 2>/dev/null || uname -s)

if [ $DISTRO = "Darwin" ]; then
      if ! brew tap | grep spacedriveapp/deps > /dev/null; then
        brew tap-new spacedriveapp/deps > /dev/null
      fi
      brew extract --force --version 5.0.1 ffmpeg spacedriveapp/deps > /dev/null
      brew unlink ffmpeg &> /dev/null || true
      brew install spacedriveapp/deps/ffmpeg@5.0.1 &> /dev/null

      echo "ffmpeg v5.0.1 has been installed and is now being used on your system."

elif [ -f /etc/debian_version -o "$DISTRO" == "Debian" -o "$DISTRO" == "Ubuntu" ]; then
        echo "Detected $DISTRO based distro!"
        DEBIAN_FFMPEG_DEPS="libavcodec-dev libavdevice-dev libavfilter-dev libavformat-dev libavutil-dev libswscale-dev libswresample-dev ffmpeg" # FFMPEG dependencies
        DEBIAN_TAURI_DEPS="libwebkit2gtk-4.0-dev build-essential curl wget libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev" # Tauri dependencies
        DEBIAN_BINDGEN_DEPS="pkg-config clang" # Bindgen dependencies - it's used by a dependency of Spacedrive

        sudo apt-get -y update
        sudo apt-get -y install ${SPACEDRIVE_CUSTOM_APT_FLAGS:-} $DEBIAN_TAURI_DEPS $DEBIAN_FFMPEG_DEPS $DEBIAN_BINDGEN_DEPS

elif [ -f /etc/os-release -o "$DISTRO" == "openSUSE" ]; then
        echo "Detected $DISTRO based distro!"
        SUSE_TAURI_DEPS="webkit2gtk3-soup2-devel libopenssl-devel curl wget libappindicator3-1 librsvg-devel" # Tauri dependencies
        SUSE_FFMPEG_DEPS="ffmpeg-4 ffmpeg-4-libavutil-devel ffmpeg-4-libavformat-devel ffmpeg-4-libswresample-devel ffmpeg-4-libavfilter-devel ffmpeg-4-libavdevice-devel" # FFMPEG dependencies
        SUSE_BINDGEN_DEPS="clang" # Bindgen dependencies - it's used by a dependency of Spacedrive

        if ! which pnpm &> /dev/null; then
                echo "PNPM was not detected on your system. Ensure the 'pnpm' command is in your \$PATH. You are not able to use Yarn or NPM." && curl -fsSL https://get.pnpm.io/install.sh | sh -
        fi
        sudo zypper up
        sudo zypper in $SUSE_TAURI_DEPS $SUSE_FFMPEG_DEPS $SUSE_BINDGEN_DEPS
        sudo zypper in -t pattern devel_basis

elif [ -f /usr/lib/os-release -o "$DISTRO" == "Arch" ]; then
        echo "Detected $DISTRO based distro!"
        ARCH_TAURI_DEPS="webkit2gtk base-devel curl wget openssl appmenu-gtk-module gtk3 libappindicator-gtk3 librsvg libvips" # Tauri deps https://tauri.studio/guides/getting-started/setup/linux#1-system-dependencies
        ARCH_FFMPEG_DEPS="ffmpeg" # FFMPEG dependencies
        ARCH_BINDGEN_DEPS="clang" # Bindgen dependencies - it's used by a dependency of Spacedrive

        sudo pacman -Syu
        sudo pacman -S --needed $ARCH_TAURI_DEPS $ARCH_FFMPEG_DEPS $ARCH_BINDGEN_DEPS

elif [ -f /etc/redhat-release -o "$DISTRO" == "RedHat" -o "$DISTRO" == "CentOS" -o "$DISTRO" == "Fedora" ]; then
        echo "Detected $DISTRO based distro!"
        FEDORA_TAURI_DEPS="webkit2gtk3-devel.x86_64 openssl-devel curl wget libappindicator-gtk3 librsvg2-devel" # Tauri dependencies
        FEDORA_FFMPEG_DEPS="ffmpeg ffmpeg-devel" # FFMPEG dependencies
        FEDORA_BINDGEN_DEPS="clang" # Bindgen dependencies - it's used by a dependency of Spacedrive

        sudo dnf check-update
        sudo dnf install $FEDORA_TAURI_DEPS $FEDORA_FFMPEG_DEPS $FEDORA_BINDGEN_DEPS
        sudo dnf group install "C Development Tools and Libraries"

else
        echo "Your Linux distro '$(lsb_release -s -d)' is not supported by this script. We would welcome a PR or some help adding your OS to this script. https://github.com/spacedriveapp/spacedrive/issues"
        exit 1
fi

echo "Your machine has been setup for Spacedrive development!"
