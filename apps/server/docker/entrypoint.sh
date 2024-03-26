#!/usr/bin/env sh

set -eu

# Shortcircuit for non-default commands.
# The last part inside the "{}" is a workaround for the following bug in ash/dash:
# https://bugs.debian.org/cgi-bin/bugreport.cgi?bug=874264
if [ -n "${1:-}" ] && [ "${1#-}" = "${1}" ] \
  && [ -n "$(command -v -- "${1}")" ] \
  && { ! [ -f "${1}" ] || [ -x "${1}" ]; }; then
  exec "$@"
fi

if [ "$(id -u)" -ne 0 ]; then
  echo "This container requires executing as root for initial setup, privileges are dropped shortly after" 1>&2
  exit 1
fi

delpasswd () {
  deluser "$@"
}

create () {
  if [ "$#" -ne 3 ] || ! { [ "$1" = "group" ] || [ "$1" = "passwd" ]; } || [ -z "$2" ] || [ "$3" -le 0 ] ; then
    echo "Usage: create <group|passwd> <NAME> <ID>" 1>&2
    echo "  NAME: Group or user name to be created" 1>&2
    echo "  ID: ID > 1000 to be assigned to the group or user" 1>&2
    exit 1
  fi

  if getent "$1" "$2" >/dev/null; then
    if [ "$(getent "$1" "$2" | cut -d: -f3)" = "$3" ]; then
      echo "$1 $2 already exists with ID: $3"
      return
    else
      "del${1}" "$2"
    fi
  fi

  if getent "$1" "$3" >/dev/null; then
    # WARNING: This need to be modified if this functions arguments are changed
    set -- "$1" "$2" "$3" "$(getent "$1" "$3" | cut -d: -f1)"
    if [ "$2" = "$4" ]; then
      echo "$1 $2 already exists with ID: $3"
      return
    else
      "del${1}" "$4"
    fi
  fi

  case "$1" in
    group)
      addgroup --system --gid "$3" "$2"
      ;;
    passwd)
      rm -rf /var/empty
      adduser \
        --system \
        --uid "$3" \
        --home /var/empty \
        --shell /bin/nologin \
        --gecos "$2 system account" \
        -G nobody \
        --no-create-home \
        "$2"
      passwd -l "$2"
      ;;
  esac
}

echo "Configure unprivileged user"
create group spacedrive 1000
create passwd spacedrive 1000

# Add spacedrive user to spacedrive group, if it is not already in it
if ! id -Gn spacedrive | tr '[:space:]+' '\n' | grep -x 'spacedrive'; then
  adduser spacedrive spacedrive
fi

if [ -n "${TZ:-}" ]; then
  echo "Set Timezone to $TZ"
  rm -f /etc/localtime
  ln -s "/usr/share/zoneinfo/${TZ}" /etc/localtime
  echo "$TZ" >/etc/timezone
fi

echo "Fix spacedrive's directories permissions"
chown -R "${PUID}:${PGID}" /data

exec su spacedrive -s /usr/bin/sd-server -- "$@"
