#!/usr/bin/env bash
# Tauri bundles the daemon as an external binary named
# target/release/sd-daemon-<target-triple>, and its build script requires that
# file to exist before `tauri dev` or `tauri build` can start. The daemon is
# built for the triple Tauri is building, so a cross build such as
# x86_64-apple-darwin on an Apple silicon runner ships a daemon the target Mac
# can run. Cargo makes the rebuild a fast no-op when the daemon is unchanged,
# so running this every time is cheap.
set -euo pipefail

cd "$(dirname "$0")/.."

target_dir="${CARGO_TARGET_DIR:-../../target}"
host="$(rustc -vV | sed -n 's/^host: //p')"
triple="${CARGO_BUILD_TARGET:-${TAURI_ENV_TARGET_TRIPLE:-$host}}"

ext=""
case "$triple" in *windows*) ext=".exe" ;; esac

# A host build stays in target/release so it shares the cache of every other
# local release build. CARGO_BUILD_TARGET makes cargo use target/<triple>
# even for the host, so the shortcut applies only without it.
build() {
	local out="$target_dir/$1/release" args=(--target "$1")
	if [ "$1" = "$host" ] && [ -z "${CARGO_BUILD_TARGET:-}" ]; then
		out="$target_dir/release"
		args=()
	fi
	cargo build --release --bin sd-daemon --manifest-path ../../Cargo.toml "${args[@]}" \
		${SD_PREVIEW_FEATURES:+--features "$SD_PREVIEW_FEATURES"}
	mkdir -p "$target_dir/release"
	cp "$out/sd-daemon$ext" "$target_dir/release/sd-daemon-$1$ext"
}

if [ "$triple" = universal-apple-darwin ]; then
	build aarch64-apple-darwin
	build x86_64-apple-darwin
	lipo -create -output "$target_dir/release/sd-daemon-$triple" \
		"$target_dir/release/sd-daemon-aarch64-apple-darwin" \
		"$target_dir/release/sd-daemon-x86_64-apple-darwin"
else
	build "$triple"
fi

echo "daemon sidecar ready: $target_dir/release/sd-daemon-$triple$ext"
