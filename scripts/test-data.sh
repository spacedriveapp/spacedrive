#!/usr/bin/env bash

set -euEo pipefail

# This script is used to download test data for Spacedrive e2e tests.

_root="$(CDPATH='' cd "$(dirname "$0")/.." && pwd -P)"
_test_data_dir="${_root}/test-data"

# Check if curl and tar are available
if ! command -v curl &>/dev/null; then
  echo "curl is required to download test data" >&2
  exit 1
fi
if ! command -v tar &>/dev/null; then
  echo "tar is required to extract test data" >&2
  exit 1
fi

rm -rf "$_test_data_dir"
mkdir "$_test_data_dir"

if [ "${1:-}" == "small" ]; then
  echo "Downloading WPT test resources..."
  curl -L# 'https://github.com/web-platform-tests/wpt/archive/refs/heads/master.tar.gz' \
    | tar -xzf - -C "$_test_data_dir" \
      wpt-master/images
else
  echo "Downloading WPT test resources..."
  curl -L# 'https://github.com/web-platform-tests/wpt/archive/refs/heads/master.tar.gz' \
    | tar -xzf - -C "$_test_data_dir" \
      wpt-master/images \
      wpt-master/jpegxl/resources

  echo "Downloading HEIF test resources..."
  curl -L# 'https://github.com/nokiatech/heif_conformance/archive/refs/heads/master.tar.gz' \
    | tar -xzf - -C "$_test_data_dir" \
      heif_conformance-master/conformance_files

  echo "Downloading WEBP test resources..."
  curl -L# 'https://github.com/webmproject/libwebp-test-data/archive/refs/heads/main.tar.gz' \
    | tar -xzf - -C "$_test_data_dir"

  echo "Downloading PNG test resources..."
  mkdir -p "${_test_data_dir}/png-test-suite"
  curl -L# 'http://www.schaik.com/pngsuite/PngSuite-2017jul19.tgz' \
    | tar -xzf - -C "${_test_data_dir}/png-test-suite"

  echo "Downloading image-rs test resources..."
  curl -L# 'https://github.com/image-rs/image/archive/refs/heads/main.tar.gz' \
    | tar -xzf - -C "$_test_data_dir" \
      image-main/tests/images/bmp \
      image-main/tests/images/gif \
      image-main/tests/images/ico \
      image-main/tests/images/tiff

  echo "Downloading chromium media test resources..."
  mkdir -p "${_test_data_dir}/chromium-media"
  curl -L# 'https://chromium.googlesource.com/chromium/src/+archive/refs/heads/main/media/test/data.tar.gz' \
    | tar -xzf - -C "${_test_data_dir}/chromium-media"

  echo "Downloading chromium pdf test resources..."
  mkdir -p "${_test_data_dir}/chromium-pdf"
  curl -L# 'https://chromium.googlesource.com/chromium/src/+archive/refs/heads/main/pdf/test/data.tar.gz' \
    | tar -xzf - -C "${_test_data_dir}/chromium-pdf"
fi

while IFS= read -r -d '' _test_file; do
  _mime_type="$(file -b --mime-type "$_test_file")"
  case "$_mime_type" in
    image/* | audio/* | video/*)
      _type_dir="${_test_data_dir}/${_mime_type%%/*}"
      ;;
    application/pdf)
      _type_dir="${_test_data_dir}/pdf"
      ;;
    *)
      continue
      ;;
  esac

  mkdir -p "$_type_dir"
  mv "$_test_file" "$_type_dir"
done < <(find "$_test_data_dir" -type f -print0)

rm -rf "${_test_data_dir}"/{wpt-master,heif_conformance-master,libwebp-test-data-main,png-test-suite,image-main,chromium-media,chromium-pdf}
