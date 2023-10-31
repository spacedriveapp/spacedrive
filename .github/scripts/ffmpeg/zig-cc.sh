#!/usr/bin/env bash

set -euo pipefail

case "$TARGET" in
  *linux-gnu)
    # Set the glibc minimum version
    # This is the lowest we can go at the moment
    # https://github.com/ziglang/zig/issues/9412
    TARGET="${TARGET}.2.18"
    ;;
esac

LD=''
case "$TARGET" in
  *linux*)
    LD='ld.lld'
    ;;
  *windows*)
    LD='lld-link'
    ;;
esac

case "$0" in
  *-cc)
    CMD='cc'
    ;;
  *-c++)
    CMD='c++'
    ;;
  *)
    echo "Unsupported mode: $0" >&2
    exit 1
    ;;
esac

args_bak=("$@")

lto=''
help=0
argv=()
stdin=0
stdout=0
l_args=()
c_argv=('-target' "$TARGET")
sysroot=''
assembler=0
has_iphone=0
preprocessor=0
assembler_file=0
should_add_libcharset=0
has_undefined_dynamic_lookup=0
while [ "$#" -gt 0 ]; do
  # Grab linker args into a separate array
  if (case "$1" in -Wl,*) exit 0 ;; *) exit 1 ;; esac) then
    IFS=',' read -ra _args <<<"$1"
    unset "_args[0]"
    l_args+=("${_args[@]}")
    shift
    continue
  fi

  if [ "$1" = '-o-' ] || [ "$1" = '-o=-' ]; then
    # -E redirect to stdout by default, -o - breaks it so we ignore it
    stdout=1
  elif [ "$1" = '-o' ] && [ "$2" = '-' ]; then
    stdout=1
    shift 2
    continue
  elif [ "$1" = '-' ]; then
    stdin=1
  elif [ "$1" = '-lgcc_s' ]; then
    # Replace libgcc_s with libunwind
    argv+=('-lunwind')
  elif [ "$1" == '-lgcc_eh' ]; then
    # zig doesn't provide gcc_eh alternative
    # https://github.com/ziglang/zig/issues/17268
    # We use libc++ to replace it
    argv+=('-lc++')
  elif [ "$1" = '-fno-lto' ]; then
    # Zig dont respect -fno-lto when -flto is set, so keep track of it here and strip it if needed
    lto='-fno-lto'
  elif [ "$1" = '-flto' ]; then
    if [ -z "$lto" ]; then
      lto="-flto=auto"
    fi
  elif (case "$1" in -flto=*) exit 0 ;; *) exit 1 ;; esac) then
    if [ "$lto" != '-fno-lto' ]; then
      lto="$1"
    fi
  elif [ "$1" = '-xassembler' ] || [ "$1" = '--language=assembler' ]; then
    # Zig behaves very oddly when passed the explicit assembler language option
    # https://github.com/ziglang/zig/issues/10915
    # https://github.com/ziglang/zig/pull/13544
    assembler=1
  elif { [ "$1" = '-x' ] || [ "$1" = '--language' ]; } && [ "${2:-}" = 'assembler' ]; then
    assembler=1
    shift 2
    continue
  elif (case "$1" in -mcpu=* | -march=*) exit 0 ;; *) exit 1 ;; esac) then
    # Ignore -mcpu and -march flags, we set them ourselves
    true
  else
    if (case "$TARGET" in *macos*) exit 0 ;; *) exit 1 ;; esac) then
      if (case "$1" in -DTARGET_OS_IPHONE*) exit 0 ;; *) exit 1 ;; esac) then
        has_iphone=1
      else
        argv+=("$1")

        # See https://github.com/apple-oss-distributions/libiconv/blob/a167071feb7a83a01b27ec8d238590c14eb6faff/xcodeconfig/libiconv.xcconfig
        if [ "$1" = '-lcharset' ]; then
          should_add_libcharset=-1
        elif [ "$1" = '-liconv' ] && [ "$should_add_libcharset" -eq 0 ]; then
          should_add_libcharset=1
        fi
      fi
    else
      argv+=("$1")
    fi
  fi

  if [ "$1" = '-E' ]; then
    preprocessor=1
  elif [ "$1" = '--help' ] || [ "$1" = '-v' ] || [ "$1" = '--version' ]; then
    help=1
  elif (case "$1" in *.S) exit 0 ;; *) exit 1 ;; esac) then
    assembler_file=1
  elif [ "$1" = '-undefined' ] && [ "${2:-}" == 'dynamic_lookup' ]; then
    argv+=("$2")
    has_undefined_dynamic_lookup=1
    shift 2
    continue
  elif (case "$1" in --sysroot=*) exit 0 ;; *) exit 1 ;; esac) then
    sysroot="$(echo "$1" | cut -d "=" -f 2-)"
  fi

  shift
done

# Ensure compiler informs linker about how to handle undefined symbols
if [ $has_undefined_dynamic_lookup -eq 1 ]; then
  l_args+=('-undefined=dynamic_lookup')
fi

# Set linker flags as global args to be parsed below
set -- "${l_args[@]}"

l_args=()
while [ "$#" -gt 0 ]; do
  if [ "$1" = '-h' ] || [ "$1" = '-help' ] || [ "$1" = '--help' ] || [ "$1" = '/?' ]; then
    if [ "$LD" == 'lld-link' ]; then
      exec "$LD" -help
    elif [ "$LD" == 'ld.lld' ]; then
      exec "$LD" --help
    else
      exec zig cc "${c_argv[@]}" --help
    fi
  elif [ "$1" = '-rpath-link' ]; then
    # zig doesn't support -rpath-link arg
    # https://github.com/ziglang/zig/pull/10948
    shift
  elif [ "$1" = '-pie' ] \
    || [ "$1" = '--pic-executable' ]; then
    # zig cc doesn't support -Wl,-pie or -Wl,--pic-executable, so we add -fPIE instead
    argv+=('-fPIE')
  elif [ "$1" = '-dylib' ] \
    || [ "$1" = '--large-address-aware' ] \
    || [ "$1" = '--no-undefined-version' ] \
    || [ "$1" = '--disable-auto-image-base' ]; then
    # zig doesn't support -dylib, --no-undefined-version, --large-address-aware and --disable-auto-image-base
    # https://github.com/ziglang/zig/issues/16855
    true
  elif [ "$1" = '-exported_symbols_list' ]; then
    # zig doesn't support -exported_symbols_list arg
    # https://clang.llvm.org/docs/ClangCommandLineReference.html#cmdoption-clang-exported_symbols_list
    shift
  elif (case "$1" in --exported_symbols_list*) exit 0 ;; *) exit 1 ;; esac) then
    true
  else
    l_args+=("$1")
  fi

  shift
done

if [ $stdout -eq 1 ] && ! [ $preprocessor -eq 1 ]; then
  echo "stdout mode is only supported for preprocessor" >&2
  exit 1
fi

# Work-around Zig not respecting -fno-lto when -flto is set
if [ -n "$lto" ]; then
  argv+=("$lto")
fi

# Macos requires -lcharset to be defined when using -liconv
if [ $should_add_libcharset -eq 1 ]; then
  argv+=('-lcharset')
fi

if [ "$help" -eq 0 ]; then
  # Compiler specific flags
  case "${TARGET:-}" in
    x86_64*)
      c_argv+=(-march=x86_64_v2)
      ;;
    aarch64-macos*)
      c_argv+=(-march=apple_m1)
      ;;
    aarch64*)
      # Raspberry Pi 3
      c_argv+=(-march=cortex_a53)
      ;;
  esac

  # If a SDK is defined ensure it is set as sysroot if is not already set
  if [ -z "$sysroot" ] && [ -d "${SDKROOT:-}" ]; then
    c_argv+=("--sysroot=${SDKROOT}")
    sysroot="$SDKROOT"
  fi

  # Some macos specific flags
  if (case "$TARGET" in *macos*) exit 0 ;; *) exit 1 ;; esac) then
    if [ "$has_iphone" -eq 0 ]; then
      c_argv+=('-DTARGET_OS_IPHONE=0')
    fi

    if [ -n "$sysroot" ]; then
      c_argv+=('-isystem' "${sysroot}/usr/include")
    fi

    if [ -n "$sysroot" ]; then
      c_argv+=("-L${sysroot}/usr/lib" "-L${sysroot}/System/Library/Frameworks")
    fi
  fi

  # Some windows-msvc specific flags
  if (case "$TARGET" in *windows-msvc) exit 0 ;; *) exit 1 ;; esac) then
    if [ -n "$sysroot" ]; then
      crt="$(CDPATH='' cd -- "${sysroot}/.." && pwd -P)"

      c_argv+=(
        '-isystem' "${crt}/include"
        '-isystem' "${sysroot}/include/ucrt"
        '-isystem' "${sysroot}/include/um"
        '-isystem' "${sysroot}/include/shared"
      )

      case "${TARGET:-}" in
        x86_64*)
          c_argv+=(
            "-L${crt}/lib/x86_64"
            "-L${sysroot}/lib/um/x86_64"
            "-L${sysroot}/lib/ucrt/x86_64"
          )
          ;;
        aarch64*)
          c_argv+=(
            "-L${crt}/lib/aarch64"
            "-L${sysroot}/lib/um/aarch64"
            "-L${sysroot}/lib/ucrt/aarch64"
          )
          ;;
      esac
    fi
  fi
fi

# Add linker args back
for arg in "${l_args[@]}"; do
  c_argv+=("-Wl,$arg")
done

# Zig's behaves very oddly when stdin is used, so we use a temporary file instead
# https://github.com/ziglang/zig/issues/10389
if [ $stdin -eq 1 ]; then
  if [ $assembler -eq 1 ]; then
    _file=$(mktemp __zig_fix_stdin.XXXXXX.S)
  else
    _file=$(mktemp __zig_fix_stdin.XXXXXX.c)
  fi

  trap 'rm -f $_file' EXIT

  cat >"$_file"

  argv+=("$_file")
elif [ $assembler -eq 1 ] && ! [ $assembler_file -eq 1 ]; then
  echo "Assembler mode without stdin or an explicit assembly file is not supported" >&2
  exit 1
fi

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

if [ "${_USING_GAS_PREPROCESSOR:-0}" -eq 0 ] && [ "$preprocessor" -eq 0 ] && {
  [ $assembler -eq 1 ] || [ $assembler_file -eq 1 ]
}; then
  _zig_out=
  _zig_err=
  if catch _zig_out _zig_err zig "$CMD" "${c_argv[@]}" "${argv[@]}"; then
    echo "$_zig_out"
    echo "$_zig_err" >&2
  else
    export _USING_GAS_PREPROCESSOR=1
    # If zig failed to compile the assembly, try again through gas-preprocessor.pl
    gas-preprocessor.pl -arch "${TARGET%%-*}" -as-type clang -- "$0" "${args_bak[@]}"
  fi
else
  exec zig "$CMD" "${c_argv[@]}" "${argv[@]}"
fi
