#!/usr/bin/env bash
# Tauri bundles the daemon and the CLI as external binaries named
# target/release/<name>-<target-triple>, and its build script requires those
# files to exist before `tauri dev` or `tauri build` can start. Both are built
# in one cargo invocation for the triple Tauri is building, with the same
# features, so the CLI in the app always matches its daemon, and a cross build
# such as x86_64-apple-darwin on an Apple silicon runner ships binaries the
# target Mac can run. Cargo makes the rebuild a fast no-op when nothing
# changed, so running this every time is cheap.
set -euo pipefail

cd "$(dirname "$0")/.."

target_dir="${CARGO_TARGET_DIR:-../../target}"
host="$(rustc -vV | sed -n 's/^host: //p')"
triple="${CARGO_BUILD_TARGET:-${TAURI_ENV_TARGET_TRIPLE:-$host}}"
sidecars=(sd-daemon sd-cli)

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
	cargo build --release --manifest-path ../../Cargo.toml \
		-p sd-core -p sd-cli --bin sd-daemon --bin sd-cli ${args[@]+"${args[@]}"} \
		${SD_DAEMON_FEATURES:+--features "$SD_DAEMON_FEATURES"}
	mkdir -p "$target_dir/release"
	for bin in "${sidecars[@]}"; do
		cp "$out/$bin$ext" "$target_dir/release/$bin-$1$ext"
	done
}

if [ "$triple" = universal-apple-darwin ]; then
	build aarch64-apple-darwin
	build x86_64-apple-darwin
	for bin in "${sidecars[@]}"; do
		lipo -create -output "$target_dir/release/$bin-$triple" \
			"$target_dir/release/$bin-aarch64-apple-darwin" \
			"$target_dir/release/$bin-x86_64-apple-darwin"
	done
else
	build "$triple"
fi

for bin in "${sidecars[@]}"; do
	echo "sidecar ready: $target_dir/release/$bin-$triple$ext"
done
