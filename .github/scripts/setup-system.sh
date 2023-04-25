#!/usr/bin/env bash

set -euo pipefail

function log_err() {
	echo "$@" >&2
}

function script_failure() {
	log_err "An error occurred $(if [ -n "${1:-}" ]; then echo "on line $1"; else echo "(unknown)"; fi)."
	log_err "Setup failed."
}

trap 'script_failure ${LINENO:-}' ERR

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

if [ "${CI:-}" != "true" ]; then
	echo "Installing Rust tools"
	cargo install cargo-watch
fi

echo

if [ "${1:-}" == "mobile" ]; then
	echo "Setting up for mobile development."

	# iOS targets
	if [[ $OSTYPE == "darwin"* ]]; then
		echo "Checking for Xcode..."
		if ! /usr/bin/xcodebuild -version >/dev/null; then
			log_err "Xcode was not detected."
			log_err "Please ensure Xcode is installed and try again."
			exit 1
		fi

		echo "Installing iOS targets for Rust..."

		rustup target add aarch64-apple-ios
		rustup target add aarch64-apple-ios-sim
		rustup target add x86_64-apple-ios # for CI
	fi

	# Android requires python
	if ! command -v python3 >/dev/null; then
		log_err "python3 command could not be found. This is required for Android mobile development."
		log_err "Ensure python3 is available in your \$PATH and try again."
		exit 1
	fi

	# Android targets
	echo "Setting up Android targets for Rust..."

	rustup target add armv7-linux-androideabi  # for arm
	rustup target add aarch64-linux-android    # for arm64
	rustup target add i686-linux-android       # for x86
	rustup target add x86_64-linux-android     # for x86_64
	rustup target add x86_64-unknown-linux-gnu # for linux-x86-64
	rustup target add aarch64-apple-darwin     # for darwin arm64 (if you have an M1 Mac)
	rustup target add x86_64-apple-darwin      # for darwin x86_64 (if you have an Intel Mac)
	rustup target add x86_64-pc-windows-gnu    # for win32-x86-64-gnu
	rustup target add x86_64-pc-windows-msvc   # for win32-x86-64-msvc

	echo "Done setting up mobile targets."
	echo
fi

if [[ $OSTYPE == "linux-gnu"* ]]; then
	if command -v apt-get >/dev/null; then
		echo "Detected apt!"
		echo "Installing dependencies with apt..."

		# Tauri dependencies
		DEBIAN_TAURI_DEPS="libwebkit2gtk-4.0-dev build-essential curl wget libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev"

		# FFmpeg dependencies
		DEBIAN_FFMPEG_DEPS="libavcodec-dev libavdevice-dev libavfilter-dev libavformat-dev libavutil-dev libswscale-dev libswresample-dev ffmpeg"

		# Webkit2gtk requires gstreamer plugins for video playback to work
		DEBIAN_VIDEO_DEPS="gstreamer1.0-libav gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly"

		# Bindgen dependencies - it's used by a dependency of Spacedrive
		DEBIAN_BINDGEN_DEPS="pkg-config clang"

		# Protobuf compiler
		DEBIAN_LIBP2P_DEPS="protobuf-compiler"

		sudo apt-get -y update
		sudo apt-get -y install ${SPACEDRIVE_CUSTOM_APT_FLAGS:-} $DEBIAN_TAURI_DEPS $DEBIAN_FFMPEG_DEPS $DEBIAN_BINDGEN_DEPS $DEBIAN_LIBP2P_DEPS $DEBIAN_VIDEO_DEPS
	elif command -v pacman >/dev/null; then
		echo "Detected pacman!"
		echo "Installing dependencies with pacman..."

		# Tauri deps https://tauri.studio/guides/getting-started/setup/linux#1-system-dependencies
		ARCH_TAURI_DEPS="webkit2gtk base-devel curl wget openssl appmenu-gtk-module gtk3 libappindicator-gtk3 librsvg libvips"

		# Webkit2gtk requires gstreamer plugins for video playback to work
		ARCH_VIDEO_DEPS="gst-libav gst-plugins-base gst-plugins-good gst-plugins-bad gst-plugins-ugly"

		# FFmpeg dependencies
		ARCH_FFMPEG_DEPS="ffmpeg"

		# Bindgen dependencies - it's used by a dependency of Spacedrive
		ARCH_BINDGEN_DEPS="clang"

		# Protobuf compiler - https://github.com/archlinux/svntogit-packages/blob/packages/protobuf/trunk/PKGBUILD provides `libprotoc`
		ARCH_LIBP2P_DEPS="protobuf"

		sudo pacman -Sy
		sudo pacman -S --needed $ARCH_TAURI_DEPS $ARCH_FFMPEG_DEPS $ARCH_BINDGEN_DEPS $ARCH_LIBP2P_DEPS $ARCH_VIDEO_DEPS
	elif command -v dnf >/dev/null; then
		echo "Detected dnf!"
		echo "Installing dependencies with dnf..."

		# `webkit2gtk4.0-devel` also provides `webkit2gtk3-devel`, it's just under a different package in fedora versions >= 37.
		# https://koji.fedoraproject.org/koji/packageinfo?tagOrder=-blocked&packageID=26162#taglist
		# https://packages.fedoraproject.org/pkgs/webkitgtk/webkit2gtk4.0-devel/fedora-38.html#provides
		FEDORA_37_TAURI_WEBKIT="webkit2gtk4.0-devel"
		FEDORA_36_TAURI_WEBKIT="webkit2gtk3-devel"

		# Tauri dependencies
		# openssl is manually declared here as i don't think openssl and openssl-devel are actually dependant on eachother
		# openssl also has a habit of being missing from some of my fresh Fedora installs - i've had to install it at least twice
		FEDORA_TAURI_DEPS="openssl-devel curl wget libappindicator-gtk3 librsvg2-devel"

		# required for building the openssl-sys crate
		FEDORA_OPENSSL_SYS_DEPS="perl-FindBin perl-File-Compare perl-IPC-Cmd perl-File-Copy"

		# FFmpeg dependencies
		FEDORA_FFMPEG_DEPS="ffmpeg ffmpeg-devel"

		# Webkit2gtk requires gstreamer plugins for video playback to work
		FEDORA_VIDEO_DEPS="gstreamer1-plugin-libav gstreamer1-plugins-base gstreamer1-plugins-good gstreamer1-plugins-good-extras gstreamer1-plugins-bad-free gstreamer1-plugins-bad-free-extras gstreamer1-plugins-ugly-free"

		# Bindgen dependencies - it's used by a dependency of Spacedrive
		FEDORA_BINDGEN_DEPS="clang"

		# Protobuf compiler
		FEDORA_LIBP2P_DEPS="protobuf-compiler"

		sudo dnf update

		if ! sudo dnf install $FEDORA_37_TAURI_WEBKIT && ! sudo dnf install $FEDORA_36_TAURI_WEBKIT; then
			log_err "We were unable to install the webkit2gtk4.0-devel/webkit2gtk3-devel package. Please open an issue if you feel that this is incorrect. https://github.com/spacedriveapp/spacedrive/issues"
			exit 1
		fi

		if ! sudo dnf install $FEDORA_FFMPEG_DEPS; then
			log_err "We were unable to install the FFmpeg and FFmpeg-devel packages. This is likely because the RPM Fusion free repository is not enabled. https://docs.fedoraproject.org/en-US/quick-docs/setup_rpmfusion/"
			exit 1
		fi

		sudo dnf install $FEDORA_TAURI_DEPS $FEDORA_BINDGEN_DEPS $FEDORA_LIBP2P_DEPS $FEDORA_VIDEO_DEPS
		sudo dnf group install "C Development Tools and Libraries"
	else
		log_err "Your Linux distro '$(lsb_release -s -d)' is not supported by this script. We would welcome a PR or some help adding your OS to this script. https://github.com/spacedriveapp/spacedrive/issues"
		exit 1
	fi
elif [[ $OSTYPE == "darwin"* ]]; then
	if ! command -v brew >/dev/null; then
		log_err "Homebrew was not found. Please install it using the instructions at https://brew.sh and try again."
		exit 1
	fi

	echo "Installing Homebrew dependencies..."

	BREW_LIBP2P_DEPS="protobuf"

	brew install -q $BREW_LIBP2P_DEPS

	if ! brew tap homebrew-ffmpeg/ffmpeg; then
		log_err "We were unable to add the homebrew-ffmpeg tap. Please ensure that you have ran `brew uninstall ffmpeg` and try again."
	fi

	if ! brew install homebrew-ffmpeg/ffmpeg/ffmpeg; then
		log_err "We were unable to install the homebrew-ffmpeg/ffmpeg package. Please ensure that you have ran `brew uninstall ffmpeg` and try again."
	fi
else
	log_err "Your OS ($OSTYPE) is not supported by this script. We would welcome a PR or some help adding your OS to this script. https://github.com/spacedriveapp/spacedrive/issues"
	exit 1
fi

echo "Your machine has been successfully set up for Spacedrive development."
