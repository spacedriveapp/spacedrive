#!/usr/bin/env bash

set -Eeumo pipefail

has () {
  command -v "$1" >/dev/null 2>&1
}

cleanup () {
  set +e
  trap '' SIGINT
  git restore --staged .
  git restore .
  jobs -p | xargs kill -SIGTERM
  kill -- -$$ 2>/dev/null
}

trap 'cleanup' SIGINT

if ! has git pnpm; then
  echo "Missing at on of the required dependencies: git, pnpm" >&2
  exit 1
fi

__dirname="$(CDPATH='' cd "$(dirname "$0")" && pwd -P)"

# Change to the root directory of the repository
cd "$__dirname/.."

if [ -n "$(git diff --name-only | xargs)" ]; then
  echo "Uncommitted changes found. Please commit your changes before running this script." >&2
  exit 1
fi

# Find the common ancestor of the current branch and main
if ! ancestor="$(git merge-base HEAD origin/main)"; then
  echo "Failed to find the common ancestor of the current branch and main." >&2
  exit 1
fi

# Run the linter and formatter for frontend
pnpm run -r lint --fix &
wait
pnpm run format &
wait

# Run clippy and formatter for backend
cargo clippy --fix --all --all-targets --all-features --allow-dirty --allow-staged
cargo fmt --all

# Add all fixes for changes made in this branch
git diff --cached --name-only  "$ancestor" | xargs git add

# Restore unrelated changes
git restore .
