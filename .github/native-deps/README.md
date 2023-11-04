# Native dependencies

## Build instructions

To build the native dependencies a `docker` or `podman` installation is required.
It is recomended to enable [`BuildKit`](https://docs.docker.com/build/buildkit/#getting-started) in docker.

Just run the following inside this directory:

```sh
$> dockier build --build-arg TARGET=<TARGET> -o . .
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

### How does the build process work?

This is losely base on https://github.com/BtbN/FFmpeg-Builds
Uses Zig for Windows and Linux building: https://github.com/ziglang/zig
Uses Clang with some modifications from osxcross for macOS builds: https://github.com/tpoechtrager/osxcross
macOS pre-packaged SDK comes from: https://github.com/joseluisq/macosx-sdks

Thanks to all the developers involved in building the dependencies used in this project

By using this you are agreeing with [Xcode license terms](https://www.apple.com/legal/sla/docs/xcode.pdf)
