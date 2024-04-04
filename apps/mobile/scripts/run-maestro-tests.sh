#!/usr/bin/env bash

set -eEuo pipefail

# Script root
_root="$(CDPATH='' cd -- "$(dirname "$0")" && pwd -P)"
_test_dir="$(CDPATH='' cd -- "${_root}/../tests" && pwd -P)"

PLATFORM="${1:-}"
DEVICE_ID=""
IOS_APP_BIN_PATH="${_root}/../ios/build/Build/Products/Release-iphonesimulator/Spacedrive.app"
case $PLATFORM in
  ios)
    DEVICE_ID="${2:-}"
    if [ -z "$DEVICE_ID" ]; then
      echo "Empty IOS emulator UUID" >&2
      exit 1
    fi

    if ! [ -e "$IOS_APP_BIN_PATH" ]; then
      echo "Invalid IOS app binary path" >&2
      exit 1
    fi
    ;;
  android)
    echo 'Android tests are not implemented yet' >&2
    exit 1
    ;;
  *)
    echo "Usage: run-maestro-tests.sh <android|ios>" >&2
    exit 1
    ;;
esac

start_app() {
  case $PLATFORM in
    ios)
      xcrun simctl bootstatus "$DEVICE_ID" -b
      open -a Simulator --args -CurrentDeviceUDID "$DEVICE_ID"
      xcrun simctl install "$DEVICE_ID" "${_root}/../ios/build/Build/Products/Release-iphonesimulator/Spacedrive.app"
      # ¯\_(ツ)_/¯
      sleep 10
      ;;
    android)
      echo 'Android tests are not implemented yet' >&2
      exit 1
      ;;
  esac
}

# https://stackoverflow.com/q/11027679#answer-59592881
# SYNTAX:
#   catch STDOUT_VARIABLE STDERR_VARIABLE COMMAND [ARG1[ ARG2[ ...[ ARGN]]]]
catch() {
  {
    IFS=$'\n' read -r -d '' "${1}"
    IFS=$'\n' read -r -d '' "${2}"
    (
      IFS=$'\n' read -r -d '' _ERRNO_
      return "$_ERRNO_"
    )
  } < <((printf '\0%s\0%d\0' "$( ( ( ({
    shift 2
    "${@}"
    echo "${?}" 1>&3-
  } | tr -d '\0' 1>&4-) 4>&2- 2>&1- | tr -d '\0' 1>&4-) 3>&1- | exit "$(cat)") 4>&1-)" "${?}" 1>&2) 2>&1)
}

run_maestro_test() {
  if [ $# -ne 1 ]; then
    echo "Usage: run_maestro_test <test_file>" >&2
    exit 1
  fi

  local i
  local retry_failed=0
  local retry_seconds
  for i in {1..6}; do
    _maestro_out=''
    _maestro_err=''

    # https://github.com/expo/expo/blob/339fa68/apps/bare-expo/scripts/start-ios-e2e-test.ts#L12
    if catch _maestro_out _maestro_err \
      env MAESTRO_DRIVER_STARTUP_TIMEOUT=120000 maestro --device "$DEVICE_ID" test "$1"; then
      # Test succeeded
      printf '%s' "$_maestro_out"
      printf '%s' "$_maestro_err" >&2
      return
    elif echo "$_maestro_err" | grep 'TimeoutException'; then
      # Test timed out
      # Kill maestro processes
      pgrep -fi maestro | xargs kill -KILL

      # Restart app if necessary
      case $PLATFORM in
        ios)
          if ! { xcrun simctl listapps booted | grep CFBundleIdentifier | grep Spacedrive; }; then
            start_app
          fi
          ;;
        android)
          echo 'Android tests are not implemented yet' >&2
          exit 1
          ;;
      esac

      # Retry
      retry_seconds=$((20 * i))
      echo "Test $1 timed out. Retrying in $retry_seconds seconds..."
      sleep $retry_seconds
    else
      # Test failed
      printf '%s' "$_maestro_out"
      printf '%s' "$_maestro_err" >&2
      if [ $retry_failed -eq 0 ]; then
        retry_failed=1
        echo "Test $1 failed. Retrying once more in 10 seconds..."
        sleep 10
      else
        return 1
      fi
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

# Start Spacedrive in the device emulator
start_app

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

if [ ${#failedTests[@]} -gt 0 ]; then
  echo "These tests failed:" >&2
  printf '%s\n' "${failedTests[@]}" >&2
  exit 1
fi
