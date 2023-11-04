#!/usr/bin/env bash

set -xeuo pipefail

exec /opt/sysroot/bin/cmake \
  -GNinja \
  -Wno-dev \
  -DCMAKE_TOOLCHAIN_FILE=/srv/toolchain.cmake \
  -DCMAKE_BUILD_TYPE=MinSizeRel \
  -DBUILD_SHARED_LIBS="${SHARED:-Off}" \
  -DCMAKE_INSTALL_PREFIX="${PREFIX:?Prefix must be defined}" \
  -DCMAKE_BUILD_WITH_INSTALL_RPATH=On \
  -DCMAKE_POSITION_INDEPENDENT_CODE=On \
  -DCMAKE_INTERPROCEDURAL_OPTIMIZATION="$([ "${LTO:-1}" -eq 1 ] && echo On || echo Off)" \
  "$@"
