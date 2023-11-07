#!/usr/bin/env -S bash -euo pipefail

echo "Download pdfium..."
mkdir -p pdfium

_tag='25.0'
case "$TARGET" in
  x86_64-windows*)
    _suffix='win64'
    ;;
  aarch64-windows*)
    # There is no binary available for Windows on ARM, so we use the x86 executable
    # which should run fine under Windows x86 emulation layer
    # https://learn.microsoft.com/en-us/windows/arm/apps-on-arm-x86-emulation
    _suffix='win32'
    ;;
  x86_64-linux*)
    _suffix='linux-x86_64'
    ;;
  aarch64-linux*)
    _suffix='linux-aarch_64'
    ;;
  x86_64-darwin*)
    _suffix='osx-x86_64'
    ;;
  aarch64-darwin*)
    _suffix='osx-aarch_64'
    ;;
esac

curl_tar "https://github.com/protocolbuffers/protobuf/releases/download/v${_tag}/protoc-${_tag}-${_suffix}.zip" "$OUT" 0

case "$TARGET" in
  *windows*)
    chmod 0755 "${OUT}/bin/protoc.exe"
    ;;
  *)
    chmod 0755 "${OUT}/bin/protoc"
    ;;
esac

rm "${OUT}/readme.txt"
