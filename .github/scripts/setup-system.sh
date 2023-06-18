#!/usr/bin/env bash

set -euo pipefail

if [ "${CI:-}" = "true" ]; then
  set -x
fi

# Force xz to use multhreaded extraction
export XZ_OPT='-T0'

SYSNAME="$(uname)"
FFMPEG_VERSION='6.0'

err() {
  for _line in "$@"; do
    echo "$@" >&2
  done
  exit 1
}

has() {
  if [ "$#" -ne 1 ]; then
    err "Usage: has <command>"
  fi

  command -v "$1" >/dev/null 2>&1
}

_gh_url="https://api.github.com/repos"
_sd_gh_path='spacedriveapp/spacedrive'
gh_curl() {
  if [ "$#" -ne 1 ]; then
    err "Usage: gh_curl <api_route>"
  fi

  url="$1"

  # Required headers for GitHub API
  set -- -LSs -H "Accept: application/vnd.github+json" -H "X-GitHub-Api-Version: 2022-11-28"

  # Add authorization header if GITHUB_TOKEN is set, to avoid being rate limited
  if [ -n "${GITHUB_TOKEN:-}" ]; then
    set -- "$@" -H "Authorization: Bearer $GITHUB_TOKEN"
  fi

  curl "$@" "$url"
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

echo "Setting up this system for Spacedrive development."
echo

# Change CWD to the directory of this script
CDPATH='' cd -- "$(dirname -- "$0")"
_script_path="$(pwd -P)"
_cargo_config="${_script_path}/../../.cargo"

rm -rf "$_cargo_config/config"

if ! has cargo; then
  err 'Rust was not found.' \
    "Ensure the 'rustc' and 'cargo' binaries are in your \$PATH." \
    'https://rustup.rs'
fi

if [ "${CI:-}" != "true" ] && [ "${spacedrive_skip_pnpm_check:-}" != "true" ]; then
  echo "checking for pnpm..."

  if ! has pnpm; then
    err 'pnpm was not found.' \
      "Ensure the 'pnpm' command is in your \$PATH." \
      'You must use pnpm for this project; yarn and npm are not allowed.' \
      'https://pnpm.io/installation'
  else
    echo "found pnpm!"
  fi
else
  echo "Skipping pnpm check."
fi

if [ "${CI:-}" != "true" ]; then
  echo "Installing Rust tools"
  cargo install cargo-watch
fi

echo

if [ "${1:-}" = "mobile" ]; then
  echo "Setting up for mobile development."

  # iOS targets
  if [ "$SYSNAME" = "Darwin" ]; then
    echo "Checking for Xcode..."
    if ! /usr/bin/xcodebuild -version >/dev/null; then
      err "Xcode was not detected." \
        "Please ensure Xcode is installed and try again."
    fi

    echo "Installing iOS targets for Rust..."

    rustup target add aarch64-apple-ios
    rustup target add aarch64-apple-ios-sim
    rustup target add x86_64-apple-ios # for CI
  fi

  # Android requires python
  if ! command -v python3 >/dev/null; then
    err 'python3 was not found.' \
      'This is required for Android mobile development.' \
      "Ensure 'python3' is available in your \$PATH and try again."
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

if [ "$SYSNAME" = "Linux" ]; then
  if has apt-get; then
    echo "Detected apt!"
    echo "Installing dependencies with apt..."

    # Tauri dependencies
    DEBIAN_TAURI_DEPS="libwebkit2gtk-4.0-dev build-essential curl wget libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf"

    # FFmpeg dependencies
    DEBIAN_FFMPEG_DEPS="libheif-dev libavcodec-dev libavdevice-dev libavfilter-dev libavformat-dev libavutil-dev libswscale-dev libswresample-dev ffmpeg"

    # Webkit2gtk requires gstreamer plugins for video playback to work
    DEBIAN_VIDEO_DEPS="gstreamer1.0-libav gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly"

    # Bindgen dependencies - it's used by a dependency of Spacedrive
    DEBIAN_BINDGEN_DEPS="pkg-config clang"

    # Protobuf compiler
    DEBIAN_LIBP2P_DEPS="protobuf-compiler"

    sudo apt-get -y update
    sudo apt-get -y install ${SPACEDRIVE_CUSTOM_APT_FLAGS:-} $DEBIAN_TAURI_DEPS $DEBIAN_FFMPEG_DEPS $DEBIAN_BINDGEN_DEPS $DEBIAN_LIBP2P_DEPS $DEBIAN_VIDEO_DEPS
  elif has pacman; then
    echo "Detected pacman!"
    echo "Installing dependencies with pacman..."

    # Tauri deps https://tauri.studio/guides/getting-started/setup/linux#1-system-dependencies
    ARCH_TAURI_DEPS="webkit2gtk base-devel curl wget openssl appmenu-gtk-module gtk3 libappindicator-gtk3 librsvg libvips patchelf"

    # Webkit2gtk requires gstreamer plugins for video playback to work
    ARCH_VIDEO_DEPS="gst-libav gst-plugins-base gst-plugins-good gst-plugins-bad gst-plugins-ugly"

    # FFmpeg dependencies
    ARCH_FFMPEG_DEPS="libheif ffmpeg"

    # Bindgen dependencies - it's used by a dependency of Spacedrive
    ARCH_BINDGEN_DEPS="clang"

    # Protobuf compiler - https://github.com/archlinux/svntogit-packages/blob/packages/protobuf/trunk/PKGBUILD provides `libprotoc`
    ARCH_LIBP2P_DEPS="protobuf"

    sudo pacman -Sy --needed $ARCH_TAURI_DEPS $ARCH_FFMPEG_DEPS $ARCH_BINDGEN_DEPS $ARCH_LIBP2P_DEPS $ARCH_VIDEO_DEPS
  elif has dnf; then
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
    FEDORA_TAURI_DEPS="openssl-devel curl wget libappindicator-gtk3 librsvg2-devel patchelf"

    # required for building the openssl-sys crate
    FEDORA_OPENSSL_SYS_DEPS="perl-FindBin perl-File-Compare perl-IPC-Cmd perl-File-Copy"

    # FFmpeg dependencies
    FEDORA_FFMPEG_DEPS="libheif-devel ffmpeg ffmpeg-devel"

    # Webkit2gtk requires gstreamer plugins for video playback to work
    FEDORA_VIDEO_DEPS="gstreamer1-plugin-libav gstreamer1-plugins-base gstreamer1-plugins-good gstreamer1-plugins-good-extras gstreamer1-plugins-bad-free gstreamer1-plugins-bad-free-extras gstreamer1-plugins-ugly-free"

    # Bindgen dependencies - it's used by a dependency of Spacedrive
    FEDORA_BINDGEN_DEPS="clang"

    # Protobuf compiler
    FEDORA_LIBP2P_DEPS="protobuf-compiler"

    if ! sudo dnf install $FEDORA_37_TAURI_WEBKIT && ! sudo dnf install $FEDORA_36_TAURI_WEBKIT; then
      err 'We were unable to install the webkit2gtk4.0-devel/webkit2gtk3-devel package.' \
        'Please open an issue if you feel that this is incorrect.' \
        'https://github.com/spacedriveapp/spacedrive/issues'
    fi

    if ! sudo dnf install $FEDORA_FFMPEG_DEPS; then
      err 'We were unable to install the FFmpeg and FFmpeg-devel packages.' \
        'This is likely because the RPM Fusion free repository is not enabled.' \
        'https://docs.fedoraproject.org/en-US/quick-docs/setup_rpmfusion'
    fi

    sudo dnf install $FEDORA_TAURI_DEPS $FEDORA_BINDGEN_DEPS $FEDORA_LIBP2P_DEPS $FEDORA_VIDEO_DEPS
    sudo dnf group install "C Development Tools and Libraries"
  else
    err "Your Linux distro '$(lsb_release -s -d)' is not supported by this script." \
      'We would welcome a PR or some help adding your OS to this script:' \
      'https://github.com/spacedriveapp/spacedrive/issues'
  fi
elif [ "$SYSNAME" = "Darwin" ]; then
  # Location for installing script dependencies
  _deps_dir="${_script_path}/deps"
  mkdir -p "$_deps_dir"
  PATH="$PATH:${_deps_dir}"
  export PATH

  _arch="$(uname -m)"

  if ! has jq; then
    echo "Download jq build..."

    # Determine the machine's architecture
    case "$_arch" in
      x86_64)
        _jq_url='https://packages.macports.org/jq/jq-1.6_4.darwin_19.x86_64.tbz2'
        _oniguruma6_url='https://packages.macports.org/oniguruma6/oniguruma6-6.9.8_0.darwin_19.x86_64.tbz2'
        ;;
      arm64)
        _jq_url='https://packages.macports.org/jq/jq-1.6_4.darwin_20.arm64.tbz2'
        _oniguruma6_url='https://packages.macports.org/oniguruma6/oniguruma6-6.9.8_0.darwin_20.arm64.tbz2'
        ;;
      *)
        err "Unsupported architecture: $_arch"
        ;;
    esac

    # Download the latest jq binary and deps from macports
    curl -LSs "$_jq_url" | tar -xjOf - ./opt/local/bin/jq >"${_deps_dir}/jq"
    curl -LSs "$_oniguruma6_url" | tar -xjOf - ./opt/local/lib/libonig.5.dylib >"${_deps_dir}/libonig.5.dylib"

    # Make the binaries executable
    chmod +x "$_deps_dir"/*

    # Make jq look for deps in the same directory
    install_name_tool -change '/opt/local/lib/libonig.5.dylib' '@executable_path/libonig.5.dylib' "${_deps_dir}/jq"
  fi

  # Create frameworks directory to put Spacedrive dependencies
  _frameworks_dir="${_script_path}/../../target/Frameworks"
  rm -rf "$_frameworks_dir"
  mkdir -p "${_frameworks_dir}/"{bin,lib,include}
  _frameworks_dir="$(CDPATH='' cd -- "$_frameworks_dir" && pwd -P)"

  exec 3>&1 # Copy stdout to fd 3.
  echo "Download ffmpeg build..."
  _page=1
  while [ $_page -gt 0 ]; do
    # TODO: Filter only actions triggered by the main branch
    _success=$(gh_curl "${_gh_url}/${_sd_gh_path}/actions/workflows/ffmpeg-macos.yml/runs?page=${_page}&per_page=100&status=success" \
      | jq -r '. as $raw | .workflow_runs | if length == 0 then error("Error: \($raw)") else .[] | .artifacts_url end' \
      | while IFS= read -r _artifacts_url; do
        if _artifact_path="$(
          gh_curl "$_artifacts_url" \
            | jq --arg version "$FFMPEG_VERSION" --arg arch "$(
              if [ "${TARGET:-}" = 'aarch64-apple-darwin' ]; then
                echo 'arm64'
              else
                echo "$_arch"
              fi
            )" -r \
              '. as $raw | .artifacts | if length == 0 then error("Error: \($raw)") else .[] | select(.name == "ffmpeg-\($version)-\($arch)") | "suites/\(.workflow_run.id)/artifacts/\(.id)" end'
        )"; then
          if {
            gh_curl "${_gh_url}/${_sd_gh_path}/actions/artifacts/$(echo "$_artifact_path" | awk -F/ '{print $4}')/zip" \
              | tar -xOf- | tar -xJf- -C "$_frameworks_dir"
          } 2>/dev/null; then
            printf 'yes'
            exit
            # nightly.link is a workaround for the lack of a public GitHub API to download artifacts from a workflow run
            # https://github.com/actions/upload-artifact/issues/51
            # Use it when running in evironments that are not authenticated with github
          elif curl -LSs "https://nightly.link/${_sd_gh_path}/${_artifact_path}" | tar -xOf- | tar -xJf- -C "$_frameworks_dir"; then
            printf 'yes'
            exit
          fi

          echo "Failed to ffmpeg artifiact release, trying again in 1sec..." >&3
          sleep 1
        fi
      done)

    if [ "${_success:-}" = 'yes' ]; then
      break
    fi

    _page=$((_page + 1))

    echo "ffmpeg artifact not found, trying again in 1sec..."
    sleep 1
  done

  # Sign and Symlink the FFMpeg.framework libs to the lib directory
  for _lib in "${_frameworks_dir}/FFMpeg.framework/Libraries/"*; do
    if [ -f "$_lib" ]; then
      # Sign the lib with the local machine certificate (Required for it to work on macOS 13+)
      if ! codesign -s "${APPLE_SIGNING_IDENTITY:--}" -f "$_lib" 1>/dev/null 2>&1; then
        err "Failed to sign: ${_lib#"$_frameworks_dir"}" \
          'Please open an issue on https://github.com/spacedriveapp/spacedrive/issues'
      fi
    fi
    _lib="${_lib#"${_frameworks_dir}/FFMpeg.framework/Libraries/"}"
    ln -s "../FFMpeg.framework/Libraries/${_lib}" "${_frameworks_dir}/lib/${_lib}"
  done

  # Symlink the FFMpeg.framework headers to the include directory
  for _header in "${_frameworks_dir}/FFMpeg.framework/Headers/"*; do
    _header="${_header#"${_frameworks_dir}/FFMpeg.framework/Headers/"}"
    ln -s "../FFMpeg.framework/Headers/${_header}" "${_frameworks_dir}/include/${_header}"
  done

  # Workaround while https://github.com/tauri-apps/tauri/pull/3934 is not merged
  echo "Download patched tauri cli.js build..."
  (
    case "$_arch" in
      x86_64)
        _artifact_id="702683038"
        ;;
      arm64)
        _artifact_id="702683035"
        ;;
      *)
        err "Unsupported architecture: $_arch"
        ;;
    esac

    if ! {
      gh_curl "${_gh_url}/${_sd_gh_path}/actions/artifacts/${_artifact_id}/zip" \
        | tar -xf- -C "${_frameworks_dir}/bin"
    } 2>/dev/null; then
      # nightly.link is a workaround for the lack of a public GitHub API to download artifacts from a workflow run
      # https://github.com/actions/upload-artifact/issues/51
      # Use it when running in evironments that are not authenticated with github
      curl -LSs "https://nightly.link/${_sd_gh_path}/actions/artifacts/${_artifact_id}.zip" \
        | tar -xf- -C "${_frameworks_dir}/bin"
    fi
  )

  echo "Download protobuf build"
  _page=1
  while [ $_page -gt 0 ]; do
    _success=$(gh_curl "${_gh_url}/protocolbuffers/protobuf/releases?page=${_page}&per_page=100" \
      | jq --arg arch "$(
        if [ "$_arch" = 'arm64' ]; then
          echo 'aarch_64'
        else
          echo 'x86_64'
        fi
      )" -r \
        '. as $raw | if length == 0 then error("Error: \($raw)") else .[] | select(.prerelease | not)  | .assets[] | select(.name | endswith("osx-\($arch).zip")) | .browser_download_url end' \
      | while IFS= read -r _asset_url; do
        if curl -LSs "${_asset_url}" | tar -xf - -C "$_frameworks_dir"; then
          printf 'yes'
          exit
        fi

        echo "Failed to download protobuf release, trying again in 1sec..." >&3
        sleep 1
      done)

    if [ "${_success:-}" = 'yes' ]; then
      break
    fi

    _page=$((_page + 1))

    echo "protobuf release not found, trying again in 1sec..."
    sleep 1
  done

  # Ensure all binaries are executable
  chmod +x "$_frameworks_dir"/bin/*

  cat <<EOF >"${_cargo_config}/config"
[env]
PROTOC = "${_frameworks_dir}/bin/protoc"
FFMPEG_DIR = "${_frameworks_dir}"

[target.aarch64-apple-darwin]
rustflags = ["-L", "${_frameworks_dir}/lib"]

[target.x86_64-apple-darwin]
rustflags = ["-L", "${_frameworks_dir}/lib"]

$(cat "${_cargo_config}/config.toml")
EOF
else
  err "Your OS ($SYSNAME) is not supported by this script." \
    'We would welcome a PR or some help adding your OS to this script.' \
    'https://github.com/spacedriveapp/spacedrive/issues'
fi

echo "Your machine has been successfully set up for Spacedrive development."
