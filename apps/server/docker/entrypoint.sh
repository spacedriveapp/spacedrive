#!/usr/bin/env sh

set -eu

# Shortcircuit for non-default commands.
# The last part inside the "{}" is a workaround for the following bug in ash/dash:
# https://bugs.debian.org/cgi-bin/bugreport.cgi?bug=874264
if [ -n "${1-}" ] && [ "${1#-}" = "${1}" ] &&
  [ -n "$(command -v -- "${1}")" ] &&
  { ! [ -f "${1}" ] || [ -x "${1}" ]; }; then
  exec "$@"
fi

if [ "$(id -u)" -ne 0 ]; then
  echo "This container requires executing as root for initial setup, privilages are dropped after" 1>&2
  exit 1
fi

echo "Configure unprivileged user"
addgroup --system --gid "${PGID}" spacedrive
adduser --system --disabled-password \
  --uid "${PUID}" \
  --home /var/empty \
  --gecos 'Spacedrive system account' \
  --ingroup spacedrive \
  spacedrive
passwd -l spacedrive

if [ -n "${TZ-}" ]; then
  echo "Set Timezone to ${TZ}"
  rm -f /etc/localtime
  ln -s "/usr/share/zoneinfo/${TZ}" /etc/localtime
  echo "${TZ}" >/etc/timezone
fi

echo "Fix spacedrive's directories permissions"
chown -R "${PUID}:${PGID}" /data

exec su spacedrive -s /bin/server -- "$@"
