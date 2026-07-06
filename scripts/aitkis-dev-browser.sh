#!/usr/bin/env bash
# Start Spacedrive for browser dev (aitkis fork).
# Usage: ./scripts/aitkis-dev-browser.sh [start|stop|status|open]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEV_BRANCH="feature/arik103"
DATA_DIR="${HOME}/.spacedrive"
DEPS_LIB="${ROOT}/apps/.deps/lib"
DAEMON_BIN="${ROOT}/target/debug/sd-daemon"
SERVER_BIN="${ROOT}/target/debug/sd-server"
WEB_DIR="${ROOT}/apps/web"
LOG_DIR="/tmp"
DAEMON_LOG="${LOG_DIR}/spacedrive-sd-daemon.log"
SERVER_LOG="${LOG_DIR}/spacedrive-dev-server.log"
VITE_LOG="${LOG_DIR}/spacedrive-vite.log"
PID_DIR="${LOG_DIR}/spacedrive-pids"

mkdir -p "$PID_DIR"

require_dev_branch() {
	local current
	current="$(git -C "$ROOT" branch --show-current 2>/dev/null || true)"
	if [[ "$current" != "$DEV_BRANCH" ]]; then
		echo "✗ Spacedrive dev must run from branch '${DEV_BRANCH}' (current: '${current:-unknown}')" >&2
		echo "  git checkout ${DEV_BRANCH}" >&2
		echo "  See docs/aitkis-fork-strategy.md" >&2
		exit 1
	fi
}

port_listening() {
	lsof -i ":$1" -sTCP:LISTEN -t >/dev/null 2>&1
}

wait_for_port() {
	local port="$1" label="$2" timeout="${3:-60}"
	for _ in $(seq 1 "$timeout"); do
		if port_listening "$port"; then
			echo "✓ ${label} listening on :${port}"
			return 0
		fi
		sleep 1
	done
	echo "✗ timed out waiting for ${label} on :${port}" >&2
	return 1
}

start_daemon() {
	if port_listening 6969; then
		echo "• daemon already on :6969"
		return 0
	fi
	[[ -x "$DAEMON_BIN" ]] || { echo "Build daemon first: cargo build --bin sd-daemon" >&2; exit 1; }
	DYLD_LIBRARY_PATH="$DEPS_LIB" nohup "$DAEMON_BIN" --data-dir "$DATA_DIR" >"$DAEMON_LOG" 2>&1 &
	echo $! >"${PID_DIR}/daemon.pid"
	wait_for_port 6969 "daemon"
}

start_server() {
	if port_listening 8080; then
		echo "• dev-server already on :8080"
		return 0
	fi
	[[ -x "$SERVER_BIN" ]] || { echo "Build server first: cargo build --bin sd-server" >&2; exit 1; }
	nohup "$SERVER_BIN" >"$SERVER_LOG" 2>&1 &
	echo $! >"${PID_DIR}/server.pid"
	wait_for_port 8080 "dev-server"
}

start_vite() {
	if port_listening 3000; then
		echo "• vite already on :3000"
		return 0
	fi
	rm -rf "${WEB_DIR}/node_modules/.vite"
	(cd "$WEB_DIR" && nohup bun run dev >"$VITE_LOG" 2>&1 &)
	echo $! >"${PID_DIR}/vite.pid"
	wait_for_port 3000 "vite" 30
}

stop_all() {
	pkill -f "${ROOT}/target/debug/sd-daemon" 2>/dev/null || true
	pkill -f "${ROOT}/target/debug/sd-server" 2>/dev/null || true
	pkill -f "${WEB_DIR}.*vite" 2>/dev/null || true
	rm -f "${PID_DIR}"/*.pid
	echo "Stopped Spacedrive browser dev services"
}

status() {
	for pair in "6969 daemon" "8080 dev-server" "3000 vite"; do
		set -- $pair
		if port_listening "$1"; then echo "✓ :$1 ($2)"; else echo "✗ :$1 ($2)"; fi
	done
	echo "--- logs ---"
	echo "daemon: ${DAEMON_LOG}"
	echo "server: ${SERVER_LOG}"
	echo "vite:   ${VITE_LOG}"
}

open_ui() {
	open -a Firefox "http://localhost:3000/" 2>/dev/null || open "http://localhost:3000/"
}

case "${1:-start}" in
	start)
		require_dev_branch
		start_daemon
		start_server
		start_vite
		echo ""
		echo "Open: http://localhost:3000"
		echo "Health: http://localhost:8080/health"
		;;
	stop) stop_all ;;
	status) status ;;
	open) open_ui ;;
	restart) require_dev_branch; stop_all; sleep 1; "$0" start ;;
	*) echo "Usage: $0 [start|stop|status|open|restart]" >&2; exit 1 ;;
esac
