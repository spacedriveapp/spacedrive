#!/usr/bin/env bash

# Creates an AppImage with the specified Rust package binary and its dependencies.
# AppImage is an universal Linux application distribution format, similar
# to macOS bundles, which packs an application and its dependencies to
# allow running it across a wide variety of distros with no system changes
# and little user hassle.
#
# Relevant documentation:
# https://docs.appimage.org/packaging-guide/index.html
# https://appimage-builder.readthedocs.io/en/latest/index.html

set -xeEuo pipefail

if [ "${CI:-}" = "true" ]; then
  set -x
fi

_root="$(CDPATH='' cd "$(dirname -- "$0")" && pwd -P)"
readonly _root

# The appimage-builder recipe to use to generate the AppImage.
readonly RECIPE="${_root}/AppImageBuilder.yml"

# The directory where the generated AppImage bundles will be stored.
readonly TARGET_APPIMAGE_DIR="${_root}/../target/${TARGET:-.}/release/bundle/appimage"
export TARGET_APPIMAGE_DIR

alias wget='wget -nc -nv --show-progress -P "$APPIMAGE_WORKDIR"'

# Create a temporary working directory for this AppImage script
APPIMAGE_WORKDIR=$(mktemp -d -t spacedrive-appimagebuild.XXX)
readonly APPIMAGE_WORKDIR
trap '{ rm -rf "$APPIMAGE_WORKDIR" || true; } && { rm -rf appimage-build AppDir || true; }' EXIT INT TERM

# Install required system dependencies
echo 'Installind required system dependencies...'
apt-get update && apt-get install -yq \
  git \
  zsync \
  dpkg-dev \
  apt-utils \
  squashfs-tools \
  libglib2.0-bin \
  gstreamer1.0-tools \
  libgdk-pixbuf2.0-bin \
  gtk-update-icon-cache

# gdk-pixbuf-query-loaders is not in PATH by default
ln -fs /usr/lib/x86_64-linux-gnu/gdk-pixbuf-2.0/gdk-pixbuf-query-loaders /usr/local/bin/gdk-pixbuf-query-loaders

if ! command -v appimage-builder >/dev/null 2>&1; then
  apt-get install -yq python3 python3-venv python3-wheel

  # Set up a virtual environment so that we do not pollute the global Python
  # packages list with the packages we need to install
  echo 'Setting up temporary Python virtual environment...'
  python3 -m venv "$APPIMAGE_WORKDIR/.venv"
  . "$APPIMAGE_WORKDIR/.venv/bin/activate"

  echo 'Install appimage-build in temporary Python virtual environment...'
  pip3 install appimage-builder
fi

echo 'Running appimage-builder...'

export TARGET_APPIMAGE_ARCH="${TARGET_APPIMAGE_ARCH:-$(uname -m)}"
export TARGET_APPIMAGE_APT_ARCH="${TARGET_APPIMAGE_APT_ARCH:-$(dpkg-architecture -q DEB_HOST_ARCH)}"
export TARGET_APPDIR="${APPIMAGE_WORKDIR}/AppDir"
export REPO_DIR="$APPIMAGE_WORKDIR/pkgs"

XDG_DATA_DIRS="$(pwd)/AppDir/usr/share:/usr/share:/usr/local/share:/var/lib/flatpak/exports/share"
export XDG_DATA_DIRS

VERSION="$(git describe --tags --dirty=-custom --always)"
export VERSION

mkdir -p "$TARGET_APPIMAGE_DIR"

appimage-builder --recipe "$RECIPE" --skip-test

echo "> Moving generated AppImage to $TARGET_APPIMAGE_DIR"
mv -f ./*.AppImage* "$TARGET_APPIMAGE_DIR"
