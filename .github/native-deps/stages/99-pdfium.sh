#!/usr/bin/env -S bash -euo pipefail

echo "Download pdfium..."
mkdir -p pdfium

_tag='chromium/6097'
case "$TARGET" in
  x86_64-windows*)
    _name='win-x64'
    ;;
  aarch64-windows*)
    _name='win-arm64'
    ;;
  x86_64-linux-gnu)
    _name='linux-x64'
    ;;
  aarch64-linux-gnu)
    _name='linux-arm64'
    ;;
  x86_64-linux-musl)
    _name='linux-musl-x64'
    ;;
  aarch64-linux-musl)
    _name='linux-musl-arm64'
    ;;
  x86_64-darwin*)
    _name='mac-x64'
    ;;
  aarch64-darwin*)
    _name='mac-arm64'
    ;;
esac

curl_tar "https://github.com/bblanchon/pdfium-binaries/releases/download/${_tag}/pdfium-${_name}.tgz" pdfium

# No src to backup here because we are downloading pre-compiled binaries

cd pdfium

# Install
mkdir -p "$OUT/include"
case "$TARGET" in
  *windows*)
    mv bin "$OUT/bin"
    mv lib/pdfium.dll.lib lib/pdfium.lib
    ;;
esac
mv lib "$OUT/lib"
mv include "$OUT/include/libpdfium"
