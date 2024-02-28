#!/usr/bin/env bash

set -eEuo pipefail

if [ "${CI:-}" = "true" ]; then
  set -x
fi

# Script root
_root="$(CDPATH='' cd -- "$(dirname "$0")" && pwd -P)"
_test_dir="$(CDPATH='' cd -- "${_root}/../tests" && pwd -P)"

PLATFORM=${1:-}
case $PLATFORM in
  ios | android) ;;
  *)
    echo "Usage: run-maestro-tests.sh <android|ios>" >&2
    exit 1
    ;;
esac

run_maestro_test() {
  if [ $# -ne 1 ]; then
    echo "Usage: run_maestro_test <test_file>" >&2
    exit 1
  fi

  local i
  local retry_seconds
  for i in {1..6}; do
    if maestro test "$1"; then
      # Test succeeded
      return
    else
      retry_seconds=$((20 * i))
      echo "Test $1 failed. Retrying in $retry_seconds seconds..."
      sleep $retry_seconds
    fi
  done

  echo "Test $1 failed after 6 retries. Exiting..." >&2
  return 1
}

# Find all test files
testFiles=()
while IFS='' read -r testFile; do testFiles+=("$testFile"); done < <(
  find "${_test_dir}" -maxdepth 1 -name '*.yml' -o -name '*.yaml'
)
if [ "$PLATFORM" == "ios" ]; then
  while IFS='' read -r testFile; do testFiles+=("$testFile"); done < <(
    find "${_test_dir}/ios-only" -name '*.yml' -o -name '*.yaml'
  )
else
  while IFS='' read -r testFile; do testFiles+=("$testFile"); done < <(
    find "${_test_dir}/android-only" -name '*.yml' -o -name '*.yaml'
  )
fi

# Run onboarding first
onboardingFile="${_test_dir}/onboarding.yml"
if ! run_maestro_test "$onboardingFile"; then
  echo "Onboarding test failed. Exiting..." >&2
  exit 1
fi

# Run the rest of the files
failedTests=()
for file in "${testFiles[@]}"; do
  # Skip onboarding.yml since it has already been run
  if [ "$file" == "$onboardingFile" ]; then
    continue
  fi

  if ! run_maestro_test "$file"; then
    failedTests+=("$file")
  fi
done

if [ ${#failedTests[@]} -eq 0 ]; then
  exit 0
else
  echo "These tests failed:" >&2
  printf '%s\n' "${failedTests[@]}" >&2
  exit 1
fi
