#!/usr/bin/env bash

set -eEuo pipefail

if [ "${CI:-}" = "true" ]; then
  set -x
fi

if [ "$(id -u)" -ne 0 ]; then
  echo "This script requires root privileges." >&2
  exec sudo -E env _UID="$(id -u)" _GID="$(id -g)" "$0" "$@"
fi

echo "Fixing deb bundle..." >&2

umask 0

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

if ! has tar curl gzip strip; then
  err 'Dependencies missing.' \
    "This script requires 'tar', 'curl', 'gzip' and 'strip' to be installed and available on \$PATH."
fi

# Go to script root
CDPATH='' cd -- "$(dirname "$0")"
_root="$(pwd -P)"

if [ -n "${TARGET:-}" ]; then
  cd "../target/${TARGET}/release/bundle/deb" || err 'Failed to find deb bundle'
else
  cd ../target/release/bundle/deb || err 'Failed to find deb bundle'
fi

# Find deb file with the highest version number, name format: spacedrive_<version>_<arch>.deb
_deb="$(find . -type f -name '*.deb' | sort -t '_' -k '2,2' -V | tail -n 1)"

# Clean up build unused artifacts
rm -rf "$(basename "$_deb" .deb)"

# Make a backup of deb
cp "$_deb" "$_deb.bak"

# Temporary directory
_tmp="$(mktemp -d)"
cleanup() {
  _err=$?

  rm -rf "$_tmp"

  # Restore backed up deb if something goes wrong
  if [ $_err -ne 0 ]; then
    mv "${_deb:?}.bak" "$_deb"
  fi

  # Ensure deb owner is the same as the user who ran the script
  chown "${_UID:-0}:${_GID:-0}" "$_deb" 2>/dev/null || true

  rm -f "${_deb:?}.bak"

  exit "$_err"
}
trap 'cleanup' EXIT

# Extract deb to a tmp dir
ar x "$_deb" --output="$_tmp"

# Extract data.tar.xz
mkdir -p "${_tmp}/data"
tar -xzf "${_tmp}/data.tar.gz" -C "${_tmp}/data"

# Extract control.tar.xz
mkdir -p "${_tmp}/control"
tar -xzf "${_tmp}/control.tar.gz" -C "${_tmp}/control"

# Fix files owner
chown -R root:root "$_tmp"

# Rename sd-desktop to spacedrive
find "${_tmp}" -name 'sd-desktop' -o \( -type f -name 'sd-desktop.*' \) | while IFS= read -r file
do
  filename="$(basename "$file")"
  if [ "$filename" = "sd-desktop" ]; then
    mv "$file" "$(dirname "$file")/spacedrive"
  else
    mv "$file" "$(dirname "$file")/spacedrive.${filename#*.}"
  fi
done

# Create doc directory
mkdir -p "$_tmp"/data/usr/share/{doc/spacedrive,man/man1}

# Create changelog.gz
curl -LSs 'https://gist.githubusercontent.com/HeavenVolkoff/0993c42bdb0b952eb5bf765398e9b921/raw/changelog' \
  | gzip -9 >"${_tmp}/data/usr/share/doc/spacedrive/changelog.gz"

# Copy LICENSE to copyright
cp "${_root}/../LICENSE" "${_tmp}/data/usr/share/doc/spacedrive/copyright"

# Copy dependencies licenses
(
  for _license in "${_root}"/../apps/.deps/licenses/*; do
    cat <<EOF
$(basename "$_license"):

$(cat "$_license")

===============================================================================

EOF
  done
) | gzip -9 >"${_tmp}/data/usr/share/doc/spacedrive/thrid-party-licenses.gz"

# Create manual page
curl -LSs 'https://gist.githubusercontent.com/HeavenVolkoff/0993c42bdb0b952eb5bf765398e9b921/raw/spacedrive.1' \
  | gzip -9 >"${_tmp}/data/usr/share/man/man1/spacedrive.1.gz"

# Fill the Categories entry in .desktop file
sed -i 's/^Categories=.*/Categories=System;FileTools;FileManager;/' "${_tmp}/data/usr/share/applications/spacedrive.desktop"
# Rename sd-desktop to spacedrive
sed -i 's/=sd-desktop/=spacedrive/' "${_tmp}/data/usr/share/applications/spacedrive.desktop"

# Fix data permissions
find "${_tmp}/data" -type d -exec chmod 755 {} +
find "${_tmp}/data" -type f -exec chmod 644 {} +

# Fix main executable permission
chmod 755 "${_tmp}/data/usr/bin/spacedrive"

# Make generic named shared libs symlinks to the versioned ones
find "${_tmp}/data/usr/lib" -type f -name '*.so.*' -exec sh -euc \
  'for _lib in "$@"; do _link="$_lib" && while { _link="${_link%.*}" && [ "$_link" != "${_lib%.so*}" ]; }; do if [ -f "$_link" ]; then ln -sf "$(basename "$_lib")" "$_link"; fi; done; done' \
  sh {} +

# Strip all executables and shared libs
find "${_tmp}/data/usr/bin" "${_tmp}/data/usr/lib" -type f -exec strip --strip-unneeded {} \;

# Add Section field to control file, if it doesnt exists
if ! grep -q '^Section:' "${_tmp}/control/control"; then
  echo 'Section: contrib/utils' >>"${_tmp}/control/control"
fi

# Add Recommends field to control file after Depends field
_recomends='gstreamer1.0-plugins-ugly'
if grep -q '^Recommends:' "${_tmp}/control/control"; then
  sed -i "s/^Recommends:.*/Recommends: ${_recomends}/" "${_tmp}/control/control"
else
  sed -i "/^Depends:/a Recommends: ${_recomends}" "${_tmp}/control/control"
fi

# Add Suggests field to control file after Recommends field
_suggests='gstreamer1.0-plugins-bad'
if grep -q '^Suggests:' "${_tmp}/control/control"; then
  sed -i "s/^Suggests:.*/Suggests: ${_suggests}/" "${_tmp}/control/control"
else
  sed -i "/^Recommends:/a Suggests: ${_suggests}" "${_tmp}/control/control"
fi

# Re-calculate md5sums
(cd "${_tmp}/data" && find . -type f -exec md5sum {} + >"${_tmp}/control/md5sums")

# Fix control files permission
find "${_tmp}/control" -type f -exec chmod 644 {} +

# Compress data.tar.xz
tar -czf "${_tmp}/data.tar.gz" -C "${_tmp}/data" .

# Compress control.tar.xz
tar -czf "${_tmp}/control.tar.gz" -C "${_tmp}/control" .

# Compress deb
ar rcs "$_deb" "${_tmp}/debian-binary" "${_tmp}/control.tar.gz" "${_tmp}/data.tar.gz"
