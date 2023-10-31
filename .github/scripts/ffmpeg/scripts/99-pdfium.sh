#!/usr/bin/env -S bash -euo pipefail

echo "Download pdfium..."
mkdir -p pdfium

_tag='chromium/6097'
case "$TARGET" in
  x86_64-windows*)
    _name='pdfium-win-x64.tgz'
    ;;
  aarch64-windows*)
    _name='pdfium-win-arm64.tgz'
    ;;
  x86_64-linux-gnu)
    _name='pdfium-linux-x64.tgz'
    ;;
  aarch64-linux-gnu)
    _name='pdfium-linux-arm64.tgz'
    ;;
  x86_64-linux-musl)
    _name='pdfium-linux-musl-x64.tgz'
    ;;
  aarch64-linux-musl)
    _name='pdfium-linux-musl-arm64.tgz'
    ;;
  x86_64-macos*)
    _name='pdfium-mac-x64.tgz'
    ;;
  aarch64-macos*)
    _name='pdfium-mac-arm64.tgz'
    ;;
esac

curl_tar "https://github.com/bblanchon/pdfium-binaries/releases/download/${_tag}/${_name}" pdfium

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
