# Native dependencies

## Build instructions

To build the native dependencies a `docker` or `podman` installation is required.
It is recomended to enable [`BuildKit`](https://docs.docker.com/build/buildkit/#getting-started) in docker.

Just run the following inside this directory:

```sh
$> docker build --build-arg TARGET=<TARGET> -o . .
```

or

```sh
$> podman build --jobs 4 --format docker --build-arg TARGET=<TARGET> -o . .
```

Where `<TARGET>` is one of:

- x86_64-darwin-apple
- aarch64-darwin-apple
- x86_64-windows-gnu
- aarch64-windows-gnu
- x86_64-linux-gnu
- aarch64-linux-gnu

After some time (it takes aroung 1~2 hours in Github CI) a directory named `out` will show up in the current directory containing our native dependencies.

### Acknowledgments

This build system is losely base on:

- https://github.com/BtbN/FFmpeg-Builds

It uses Zig 0.11 as a C/C++ toolchain to build the Windows and Linux targets:

- https://github.com/ziglang/zig

It uses LLVM/Clang 16 with some tweaks from osxcross + Apple's cctools and linker to build the Darwin (macOS) targets:

- https://github.com/tpoechtrager/osxcross
- https://github.com/tpoechtrager/cctools-port

The pre-packaged macOS SDK comes from: https://github.com/joseluisq/macosx-sdks

> By building the Darwin target you are agreeing with the [Apple Public Source License (APSL)](https://opensource.apple.com/apsl/) and the [Xcode license terms](https://www.apple.com/legal/sla/docs/xcode.pdf)

Thanks to all the developers involved in making the dependencies used in this project
