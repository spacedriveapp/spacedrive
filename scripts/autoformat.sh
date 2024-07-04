#!/usr/bin/env bash

set -Eeumo pipefail

has() {
  command -v "$1" >/dev/null 2>&1
}

handle_exit() {
  _exit=$?
  set +e
  trap '' SIGINT
  trap - EXIT
  if [ "$_exit" -ne 0 ]; then
    git restore --staged .
    git restore .
  fi
  exit "$_exit"
}

cleanup() {
  set +e
  trap '' SIGINT
  trap - EXIT
  jobs -p | xargs kill -SIGTERM
  git restore --staged .
  git restore .
  kill -- -$$ 2>/dev/null
}

if ! has git pnpm; then
  echo "Missing at on of the required dependencies: git, pnpm" >&2
  exit 1
fi

__dirname="$(CDPATH='' cd "$(dirname "$0")" && pwd -P)"

# Change to the root directory of the repository
cd "$__dirname/.."

if [ -n "$(git diff --name-only HEAD)" ] || [ -n "$(git ls-files --others --exclude-standard)" ]; then
  echo "Uncommitted changes found. Please commit or stash your changes before running this script." >&2
  exit 1
fi

# Find the common ancestor of the current branch and main
if ! {
  if [ -n "${CI:-}" ]; then
    # On CI use custom variable
    ancestor="$(git merge-base HEAD "HEAD~$((${PR_FETCH_DEPTH:?Missing PR_FETCH_DEPTH} - 1))")"
  else
    ancestor="$(git merge-base HEAD origin/main)"
  fi
}; then
  echo "Failed to find the common ancestor of the current branch and main." >&2
  exit 1
fi

# Handle errors and cleanup after formating has started
trap 'handle_exit' EXIT
trap 'cleanup' SIGINT

# Run the linter and formatter for frontend
# Use a background processes to avoid pnpm weird handling of CTRL+C
pnpm run -r lint --fix &
wait
pnpm run format &
wait

if [ "${1:-}" != "only-frontend" ]; then
  # Run clippy and formatter for backend
  cargo clippy --fix --all --all-targets --all-features --allow-dirty --allow-staged
  cargo fmt --all
fi

# Add all fixes for changes made in this branch
git diff --diff-filter=d --cached --name-only "${ancestor:?Ancestor is not set}" | xargs git add

# Restore unrelated changes
git restore .
