#!/bin/bash

set -e

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)

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

# We can always add in additional distros as needed
KNOWN_DISTRO="(Debian|Ubuntu|RedHat|CentOS|opensuse-leap|Arch|Fedora|suse)"
# This is used to identify the distro based off of the /etc/os-release file
DISTRO=$(awk -F= '$1=="ID" { print $2 ;}' /etc/os-release 2>/dev/null | grep -Eo $KNOWN_DISTRO || grep -Eo $KNOWN_DISTRO /etc/issue 2>/dev/null || uname -s | grep -Eo $KNOWN_DISTRO || grep -Eo $KNOWN_DISTRO /etc/issue 2>/dev/null || uname -s)

# shellcheck disable=SC2166
if [ "$DISTRO" = "Darwin" ]; then
  echo "Detected $DISTRO based distro!"
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

elif
  [ -f /etc/debian_version -o $DISTRO == "Debian" -o "$DISTRO" == "Ubuntu" ]
then
  echo "Detected $DISTRO based distro!"
  # FFMPEG dependencies
  DEBIAN_FFMPEG_DEPS="libavcodec-dev libavdevice-dev libavfilter-dev libavformat-dev libavutil-dev libswscale-dev libswresample-dev ffmpeg"
  # Tauri dependencies
  DEBIAN_TAURI_DEPS="libwebkit2gtk-4.0-dev build-essential curl wget libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev"
  # Bindgen dependencies - it's used by a dependency of Spacedrive
  DEBIAN_BINDGEN_DEPS="pkg-config clang"

  sudo apt-get -y update
  sudo apt-get -y install "${SPACEDRIVE_CUSTOM_APT_FLAGS:-}" $DEBIAN_TAURI_DEPS $DEBIAN_FFMPEG_DEPS $DEBIAN_BINDGEN_DEPS

elif [ -f /etc/os-release -o $DISTRO == "opensuse" ]; then
  echo "Detected $DISTRO based distro!"
  # Tauri dependencies
  SUSE_TAURI_DEPS="webkit2gtk3-soup2-devel libopenssl-devel curl wget libappindicator3-1 librsvg-devel"
  # FFMPEG dependencies
  SUSE_FFMPEG_DEPS="ffmpeg-5 ffmpeg-5-libavutil-devel ffmpeg-5-libavformat-devel ffmpeg-5-libswresample-devel ffmpeg-5-libavfilter-devel ffmpeg-5-libavdevice-devel"
  # Bindgen dependencies - it's used by a dependency of Spacedrive
  SUSE_BINDGEN_DEPS="clang"

  sudo zypper up
  sudo zypper addrepo https://download.opensuse.org/repositories/multimedia:libs/15.4/multimedia:libs.repo
  sudo zypper refresh
  sudo zypper in -t pattern $SUSE_TAURI_DEPS $SUSE_FFMPEG_DEPS $SUSE_BINDGEN_DEPS
  sudo zypper in -t pattern devel_basis

elif [ -f /usr/lib/os-release -o "$DISTRO" == "Arch" ]; then
  echo "Detected $DISTRO based distro!"
  # Tauri deps https://tauri.studio/guides/getting-started/setup/linux#1-system-dependencies
  ARCH_TAURI_DEPS="webkit2gtk base-devel curl wget openssl appmenu-gtk-module gtk3 libappindicator-gtk3 librsvg libvips"
  # FFMPEG dependencies
  ARCH_FFMPEG_DEPS="ffmpeg"
  # Bindgen dependencies - it's used by a dependency of Spacedrive
  ARCH_BINDGEN_DEPS="clang"

  sudo pacman -Syu
  sudo pacman -S --needed $ARCH_TAURI_DEPS $ARCH_FFMPEG_DEPS $ARCH_BINDGEN_DEPS

elif [ -f /etc/redhat-release -o "$DISTRO" == "RedHat" -o "$DISTRO" == "CentOS" -o "$DISTRO" == "Fedora" ]; then
  echo "Detected $DISTRO based distro!"
  # Tauri dependencies
  FEDORA_TAURI_DEPS="webkit2gtk3-devel.x86_64 openssl-devel curl wget libappindicator-gtk3 librsvg2-devel"
  # FFMPEG dependencies
  FEDORA_FFMPEG_DEPS="ffmpeg ffmpeg-devel"
  # Bindgen dependencies - it's used by a dependency of Spacedrive
  FEDORA_BINDGEN_DEPS="clang"

  sudo dnf check-update
  sudo dnf install $FEDORA_TAURI_DEPS $FEDORA_FFMPEG_DEPS $FEDORA_BINDGEN_DEPS
  sudo dnf group install "C Development Tools and Libraries"

else
#  Updated to be more precise as lsb_release is not installed by default on all distros and also less specific.
  echo "Your Linux distro $(awk -F= '$1=="PRETTY_NAME" { print $2 }' /etc/os-release) is not supported by this script. We would welcome a PR or some help adding your OS to this script. https://github.com/spacedriveapp/spacedrive/issues"
  exit 1
fi

echo "Your machine has been set up for Spacedrive development!"
