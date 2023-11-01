#!/usr/bin/env bash

set -euo pipefail

if [ "${CI:-}" = "true" ]; then
  set -x
fi

err() {
  for _line in "$@"; do
    echo "$_line" >&2
  done
  exit 1
}

has() {
  for prog in "$@"; do
    if ! command -v "$prog" 1>/dev/null 2>&1; then
      return 1
    fi
  done
}

script_failure() {
  if [ -n "${1:-}" ]; then
    _line="on line $1"
  else
    _line="(unknown)"
  fi
  err "An error occurred $_line." "Setup failed."
}

trap 'script_failure ${LINENO:-}' ERR

case "${OSTYPE:-}" in
  'msys' | 'mingw' | 'cygwin')
    err 'Bash for windows is not supported, please interact with this repo from Powershell or CMD'
    ;;
esac

if [ "${CI:-}" != "true" ]; then
  echo 'Spacedrive Development Environment Setup'
  echo 'To set up your machine for Spacedrive development, this script will install some required dependencies with your system package manager'
  echo
  echo 'Press Enter to continue'
  read -r

  if ! has pnpm; then
    err 'pnpm was not found.' \
      "Ensure the 'pnpm' command is in your \$PATH." \
      'You must use pnpm for this project; yarn and npm are not allowed.' \
      'https://pnpm.io/installation'
  fi

  if ! has rustc cargo; then
    err 'Rust was not found.' \
      "Ensure the 'rustc' and 'cargo' binaries are in your \$PATH." \
      'https://rustup.rs'
  fi

  echo "Installing Rust tools..."
  cargo install cargo-watch

  echo
fi

# Install rust deps for android
if [ "${1:-}" = "mobile" ]; then
  MOBILE=1
  # Android requires python
  if ! { has python3 || { has python && python -c 'import sys; exit(0 if sys.version_info[0] == 3 else 1)'; }; }; then
    err 'python3 was not found.' \
      'This is required for Android mobile development.' \
      "Ensure 'python3' is available in your \$PATH and try again."
  fi

  if ! has rustup; then
    err 'Rustup was not found. It is required for cross-compiling rust to mobile targets.' \
      "Ensure the 'rustup' binary is in your \$PATH." \
      'https://rustup.rs'
  fi

  # Android targets
  echo "Installing Android targets for Rust..."

  rustup target add armv7-linux-androideabi  # for arm
  rustup target add aarch64-linux-android    # for arm64
  rustup target add i686-linux-android       # for x86
  rustup target add x86_64-linux-android     # for x86_64
  rustup target add x86_64-unknown-linux-gnu # for linux-x86-64
  rustup target add aarch64-apple-darwin     # for darwin arm64 (if you have an M1 Mac)
  rustup target add x86_64-apple-darwin      # for darwin x86_64 (if you have an Intel Mac)
  rustup target add x86_64-pc-windows-gnu    # for win32-x86-64-gnu
  rustup target add x86_64-pc-windows-msvc   # for win32-x86-64-msvc

  echo
else
  MOBILE=0
fi

# Install system deps
case "$(uname)" in
  "Darwin")
    if [ "$(uname -m)" = 'x86_64' ]; then (
      if [ "${CI:-}" = "true" ]; then
        export NONINTERACTIVE=1
      fi
      brew install nasm
    ); fi

    # Install rust deps for iOS
    if [ $MOBILE -eq 1 ]; then
      echo "Checking for Xcode..."
      if ! /usr/bin/xcodebuild -version >/dev/null; then
        err "Xcode was not detected." \
          "Please ensure Xcode is installed and try again."
      fi

      echo "Installing iOS targets for Rust..."

      rustup target add aarch64-apple-ios
      rustup target add aarch64-apple-ios-sim
      rustup target add x86_64-apple-ios # for CI

      echo
    fi
    ;;
  "Linux") # https://github.com/tauri-apps/tauri-docs/blob/dev/docs/guides/getting-started/prerequisites.md#setting-up-linux
    if has apt-get; then
      echo "Detected apt!"
      echo "Installing dependencies with apt..."

      # Tauri dependencies
      set -- build-essential curl wget file patchelf openssl libssl-dev libgtk-3-dev librsvg2-dev \
        libwebkit2gtk-4.0-dev libayatana-appindicator3-dev

      # Webkit2gtk requires gstreamer plugins for video playback to work
      set -- "$@" gstreamer1.0-alsa gstreamer1.0-gl gstreamer1.0-gtk3 gstreamer1.0-libav \
        gstreamer1.0-pipewire gstreamer1.0-plugins-bad gstreamer1.0-plugins-base \
        gstreamer1.0-plugins-good gstreamer1.0-plugins-ugly gstreamer1.0-pulseaudio \
        gstreamer1.0-vaapi libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
        libgstreamer-plugins-bad1.0-dev

      # C/C++ build dependencies, required to build some *-sys crates
      set -- "$@" llvm-dev libclang-dev clang nasm

      sudo apt-get -y update
      sudo apt-get -y install "$@"
    elif has pacman; then
      echo "Detected pacman!"
      echo "Installing dependencies with pacman..."

      # Tauri dependencies
      set -- base-devel curl wget file patchelf openssl gtk3 librsvg webkit2gtk libayatana-appindicator

      # Webkit2gtk requires gstreamer plugins for video playback to work
      set -- "$@" gst-libav gst-plugins-bad gst-plugins-base gst-plugins-good gst-plugins-ugly \
        gst-plugin-pipewire gstreamer-vaapi

      # C/C++ build dependencies, required to build some *-sys crates
      set -- "$@" clang nasm

      # React dependencies
      set -- "$@" libvips

      sudo pacman -Sy --needed "$@"
    elif has dnf; then
      echo "Detected dnf!"
      echo "Installing dependencies with dnf..."

      # For Enterprise Linux, you also need "Development Tools" instead of "C Development Tools and Libraries"
      if ! { sudo dnf group install "C Development Tools and Libraries" || sudo sudo dnf group install "Development Tools"; }; then
        err 'We were unable to install the "C Development Tools and Libraries"/"Development Tools" package.' \
          'Please open an issue if you feel that this is incorrect.' \
          'https://github.com/spacedriveapp/spacedrive/issues'
      fi

      # For Fedora 36 and below, and all Enterprise Linux Distributions, you need to install webkit2gtk3-devel instead of webkit2gtk4.0-devel
      if ! { sudo dnf install webkit2gtk4.0-devel || sudo dnf install webkit2gtk3-devel; }; then
        err 'We were unable to install the webkit2gtk4.0-devel/webkit2gtk3-devel package.' \
          'Please open an issue if you feel that this is incorrect.' \
          'https://github.com/spacedriveapp/spacedrive/issues'
      fi

      # Tauri dependencies
      set -- openssl curl wget file patchelf libappindicator-gtk3-devel librsvg2-devel

      # Webkit2gtk requires gstreamer plugins for video playback to work
      set -- "$@" gstreamer1-devel gstreamer1-plugins-base-devel \
        gstreamer1-plugins-good gstreamer1-plugins-good-gtk \
        gstreamer1-plugins-good-extras gstreamer1-plugins-ugly-free \
        gstreamer1-plugins-bad-free gstreamer1-plugins-bad-free-devel \
        gstreamer1-plugins-bad-free-extras

      # C/C++ build dependencies, required to build some *-sys crates
      set -- "$@" clang clang-devel nasm

      sudo dnf install "$@"
    else
      if has lsb_release; then
        _distro="'$(lsb_release -s -d)' "
      fi
      err "Your Linux distro ${_distro:-}is not supported by this script." \
        'We would welcome a PR or some help adding your OS to this script:' \
        'https://github.com/spacedriveapp/spacedrive/issues'
    fi
    ;;
  *)
    err "Your OS ($(uname)) is not supported by this script." \
      'We would welcome a PR or some help adding your OS to this script.' \
      'https://github.com/spacedriveapp/spacedrive/issues'
    ;;
esac

echo 'Your machine has been setup for Spacedrive development!'
