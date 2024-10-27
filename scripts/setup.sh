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

sudo() {
  if [ "$(id -u)" -eq 0 ]; then
    "$@"
  else
    env sudo "$@"
  fi
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

  if [ "${CI:-}" = "true" ]; then
    # TODO: This need to be adjusted for future mobile release CI
    rustup target add x86_64-linux-android
  else
    rustup target add \
      aarch64-linux-android \
      armv7-linux-androideabi \
      x86_64-linux-android
  fi

  echo
else
  MOBILE=0
fi

# Install system deps
case "$(uname)" in
  "Darwin")
    if [ "$(uname -m)" = 'x86_64' ] && ! [ "${CI:-}" = "true" ]; then
      brew install nasm
    fi

    # Install rust deps for iOS
    if [ $MOBILE -eq 1 ]; then
      echo "Checking for Xcode..."
      if ! /usr/bin/xcodebuild -version >/dev/null; then
        err "Xcode was not detected." \
          "Please ensure Xcode is installed and try again."
      fi

      echo "Installing iOS targets for Rust..."

      case "$(uname -m)" in
        "arm64" | "aarch64") # M series
          rustup target add aarch64-apple-ios aarch64-apple-ios-sim
          ;;
        "x86_64") # Intel
          rustup target add x86_64-apple-ios aarch64-apple-ios
          ;;
        *)
          err 'Unsupported architecture for CI build.'
          ;;
      esac

      echo
    fi
    ;;
  "Linux")
    # https://github.com/tauri-apps/tauri-docs/blob/dev/docs/guides/getting-started/prerequisites.md#setting-up-linux
    if has apt-get; then
      echo "Detected apt!"
      echo "Installing dependencies with apt..."

      # Tauri dependencies
      set -- build-essential curl wget file openssl libssl-dev libgtk-3-dev librsvg2-dev \
        libwebkit2gtk-4.1-dev libayatana-appindicator3-dev libxdo-dev libdbus-1-dev

      # Webkit2gtk requires gstreamer plugins for video playback to work
      set -- "$@" gstreamer1.0-plugins-good gstreamer1.0-plugins-ugly libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev

      # C/C++ build dependencies, required to build some *-sys crates
      set -- "$@" llvm-dev libclang-dev clang nasm perl

      # React dependencies
      set -- "$@" libvips42

      sudo apt-get -y update
      sudo apt-get -y install "$@"
    elif has pacman; then
      echo "Detected pacman!"
      echo "Installing dependencies with pacman..."

      # Tauri dependencies
      set -- base-devel curl wget file openssl gtk3 librsvg webkit2gtk-4.1 libayatana-appindicator xdotool dbus

      # Webkit2gtk requires gstreamer plugins for video playback to work
      set -- "$@" gst-plugins-base gst-plugins-good gst-plugins-ugly

      # C/C++ build dependencies, required to build some *-sys crates
      set -- "$@" clang nasm perl

      # React dependencies
      set -- "$@" libvips

      sudo pacman -Sy --needed "$@"
    elif has dnf; then
      echo "Detected dnf!"
      echo "Installing dependencies with dnf..."

      # For Enterprise Linux, you also need "Development Tools" instead of "C Development Tools and Libraries"
      if ! { sudo dnf group install "C Development Tools and Libraries" || sudo dnf group install "Development Tools"; }; then
        err 'We were unable to install the "C Development Tools and Libraries"/"Development Tools" package.' \
          'Please open an issue if you feel that this is incorrect.' \
          'https://github.com/spacedriveapp/spacedrive/issues'
      fi

      # Tauri dependencies
      set -- openssl webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel librsvg2-devel libxdo-devel dbus-devel

      # Webkit2gtk requires gstreamer plugins for video playback to work
      set -- "$@" gstreamer1-devel gstreamer1-plugins-base-devel gstreamer1-plugins-good \
        gstreamer1-plugins-good-extras gstreamer1-plugins-ugly-free

      # C/C++ build dependencies, required to build some *-sys crates
      set -- "$@" clang clang-devel nasm perl-core

      # React dependencies
      set -- "$@" vips

      sudo dnf install "$@"
    elif has apk; then
      echo "Detected apk!"
      echo "Installing dependencies with apk..."
      echo "Alpine suport is experimental" >&2

      # Tauri dependencies
      set -- build-base curl wget file openssl-dev gtk+3.0-dev librsvg-dev \
        webkit2gtk-4.1-dev libayatana-indicator-dev xdotool-dev dbus-dev

      # Webkit2gtk requires gstreamer plugins for video playback to work
      set -- "$@" gst-plugins-base-dev gst-plugins-good gst-plugins-ugly

      # C/C++ build dependencies, required to build some *-sys crates
      set -- "$@" llvm16-dev clang16 nasm perl

      # React dependencies
      set -- "$@" vips

      sudo apk add "$@"
    elif has eopkg; then
      echo "Detected eopkg!"
      echo "Installing dependencies with eopkg..."
      echo "Solus support is experimental" >&2

      # Tauri dependencies
      set -- curl wget file openssl openssl-devel libgtk-3-devel librsvg-devel \
        libwebkit-gtk41-devel libayatana-appindicator-devel xdotool-devel dbus-devel

      # Webkit2gtk requires gstreamer plugins for video playback to work
      set -- "$@" gstreamer-1.0-plugins-good gstreamer-1.0-plugins-ugly gstreamer-1.0-devel gstreamer-1.0-plugins-base-devel

      # C/C++ build dependencies, required to build some *-sys crates
      set -- "$@" llvm-devel llvm-clang-devel llvm-clang nasm perl

      # React dependencies
      set -- "$@" libvips

      sudo eopkg it -c system.devel -y
      sudo eopkg it "$@" -y
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

if [ "${CI:-}" != "true" ]; then
  echo "Installing Rust tools..."

  _tools="cargo-watch"
  if [ $MOBILE -eq 1 ]; then
    _tools="$_tools cargo-ndk" # For building Android
  fi

  echo "$_tools" | xargs cargo install
fi

echo 'Your machine has been setup for Spacedrive development!'
