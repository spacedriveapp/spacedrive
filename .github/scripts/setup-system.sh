#!/bin/bash

set -e

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

function log_err {
	echo "$@" >&2
}

function script_failure {
	log_err "An error occurred$([ -z "$1" ] && " on line $1" || " (unknown)")."
	log_err "Setup failed."
}

trap 'script_failure $LINENO' ERR

echo "Setting up this system for Spacedrive development."
echo

if ! command -v cargo >/dev/null; then
	log_err "Rust was not found. Ensure the 'rustc' and 'cargo' binaries are in your \$PATH."
	exit 1
fi

if [ "${SPACEDRIVE_SKIP_PNPM_CHECK:-'false'}" != "true" ]; then
	echo "Checking for pnpm..."

	if ! command -v pnpm >/dev/null; then
		log_err "pnpm was not found. Ensure the 'pnpm' command is in your \$PATH."
		log_err 'You MUST use pnpm for this project; yarn and npm are not allowed.'
		exit 1
	else
		echo "Found pnpm!"
	fi
else
	echo "Skipping pnpm check."
fi

echo

if [ "$1" == "mobile" ]; then
	echo "Setting up for mobile development."

	# iOS targets
	if [[ "$OSTYPE" == "darwin"* ]]; then
		echo "Checking for Xcode..."
		if ! /usr/bin/xcodebuild -version >/dev/null; then
			log_err "Xcode was not detected."
			log_err "Please ensure Xcode is installed and try again."
			exit 1
		fi

		echo "Installing iOS targets for Rust..."

		rustup target add aarch64-apple-ios
		rustup target add aarch64-apple-ios-sim
	fi

	# Android requires python
	if ! command -v python3 >/dev/null
	then
		log_err "python3 command could not be found. This is required for Android mobile development."
		log_err "Ensure python3 is available in your \$PATH and try again."
		exit 1
	fi

	# Android targets
	echo "Setting up Android targets for Rust..."

	rustup target add armv7-linux-androideabi   # for arm
	rustup target add aarch64-linux-android     # for arm64
	rustup target add i686-linux-android        # for x86
	rustup target add x86_64-linux-android      # for x86_64
	rustup target add x86_64-unknown-linux-gnu  # for linux-x86-64
	rustup target add aarch64-apple-darwin      # for darwin arm64 (if you have an M1 Mac)
	rustup target add x86_64-apple-darwin       # for darwin x86_64 (if you have an Intel Mac)
	rustup target add x86_64-pc-windows-gnu     # for win32-x86-64-gnu
	rustup target add x86_64-pc-windows-msvc    # for win32-x86-64-msvc

	echo "Done setting up mobile targets."
	echo
fi

if [[ "$OSTYPE" == "linux-gnu"* ]]; then
	if command -v apt-get >/dev/null; then
		echo "Detected apt!"
		echo "Installing dependencies with apt..."
		
		DEBIAN_TAURI_DEPS="libwebkit2gtk-4.0-dev build-essential curl wget libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev" # Tauri dependencies
		DEBIAN_FFMPEG_DEPS="libavcodec-dev libavdevice-dev libavfilter-dev libavformat-dev libavutil-dev libswscale-dev libswresample-dev ffmpeg" # FFmpeg dependencies
		DEBIAN_BINDGEN_DEPS="pkg-config clang" # Bindgen dependencies - it's used by a dependency of Spacedrive

		sudo apt-get -y update
		sudo apt-get -y install ${SPACEDRIVE_CUSTOM_APT_FLAGS:-} $DEBIAN_TAURI_DEPS $DEBIAN_FFMPEG_DEPS $DEBIAN_BINDGEN_DEPS

		# TODO: Cleanup once working
		# sudo apt-get -y install libraw1394-dev libavc1394-dev librom1394-dev libiec61883-dev libjack-dev libopenal-dev libxcb-shape-dev

		# https://trac.ffmpeg.org/wiki/CompilationGuide/Ubuntu
		sudo apt-get update -qq && sudo apt-get -y install \
			autoconf \
			automake \
			build-essential \
			cmake \
			git-core \
			libass-dev \
			libfreetype6-dev \
			libgnutls28-dev \
			libmp3lame-dev \
			libsdl2-dev \
			libtool \
			libva-dev \
			libvdpau-dev \
			libvorbis-dev \
			libxcb1-dev \
			libxcb-shm0-dev \
			libxcb-xfixes0-dev \
			meson \
			ninja-build \
			pkg-config \
			texinfo \
			wget \
			yasm \
			zlib1g-dev

		sudo apt install libunistring-dev libaom-dev libdav1d-dev

		sudo apt-get install libx264-dev
		sudo apt-get install libx265-dev libnuma-dev
		sudo apt-get install libvpx-dev
		sudo apt-get install libfdk-aac-dev
		sudo apt-get install libopus-dev
		sudo apt-get install libdav1d-dev
	elif command -v pacman >/dev/null; then
		echo "Detected pacman!"
		echo "Installing dependencies with pacman..."

		ARCH_TAURI_DEPS="webkit2gtk base-devel curl wget openssl appmenu-gtk-module gtk3 libappindicator-gtk3 librsvg libvips" # Tauri deps https://tauri.studio/guides/getting-started/setup/linux#1-system-dependencies
		ARCH_FFMPEG_DEPS="ffmpeg" # FFmpeg dependencies
		ARCH_BINDGEN_DEPS="clang" # Bindgen dependencies - it's used by a dependency of Spacedrive

		sudo pacman -Syu
		sudo pacman -S --needed $ARCH_TAURI_DEPS $ARCH_FFMPEG_DEPS $ARCH_BINDGEN_DEPS
	elif command -v dnf >/dev/null; then
		echo "Detected dnf!"
		echo "Installing dependencies with dnf..."

		FEDORA_TAURI_DEPS="webkit2gtk3-devel.x86_64 openssl-devel curl wget libappindicator-gtk3 librsvg2-devel" # Tauri dependencies
		FEDORA_FFMPEG_DEPS="ffmpeg ffmpeg-devel" # FFmpeg dependencies
		FEDORA_BINDGEN_DEPS="clang" # Bindgen dependencies - it's used by a dependency of Spacedrive

		sudo dnf check-update
		sudo dnf install $FEDORA_TAURI_DEPS $FEDORA_FFMPEG_DEPS $FEDORA_BINDGEN_DEPS
		sudo dnf group install "C Development Tools and Libraries"
	else
		log_err "Your Linux distro '$(lsb_release -s -d)' is not supported by this script. We would welcome a PR or some help adding your OS to this script. https://github.com/spacedriveapp/spacedrive/issues"
		exit 1
	fi
elif [[ "$OSTYPE" == "darwin"* ]]; then
		if ! command -v brew >/dev/null; then
			log_err "Homebrew was not found. Please install it using the instructions at https://brew.sh and try again."
			exit 1
		fi

		echo "Installing Homebrew dependencies..."

		if ! brew tap -q | grep -qx "spacedriveapp/deps" >/dev/null; then
			echo "Creating Homebrew tap \`spacedriveapp/deps\`..."
			brew tap-new spacedriveapp/deps
		fi

		FFMPEG_VERSION="5.0.1"

		if ! brew list --full-name -1 | grep -x "spacedriveapp/deps/ffmpeg@$FFMPEG_VERSION" >/dev/null; then
			echo "Extracting FFmpeg version $FFMPEG_VERSION..."

			brew extract -q --force --version $FFMPEG_VERSION ffmpeg spacedriveapp/deps
			brew unlink -q ffmpeg || true
			brew install -q "spacedriveapp/deps/ffmpeg@$FFMPEG_VERSION"

			echo "FFmpeg version $FFMPEG_VERSION has been installed and is now being used on your system."
		fi
else
	log_err "Your OS ($OSTYPE) is not supported by this script. We would welcome a PR or some help adding your OS to this script. https://github.com/spacedriveapp/spacedrive/issues"
	exit 1
fi

echo "Your machine has been successfully set up for Spacedrive development."
