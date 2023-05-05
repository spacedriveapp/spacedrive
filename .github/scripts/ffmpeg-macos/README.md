# FFMpeg.framework

## Build instructions

To build `FFMpeg.framework` a `docker` or `podman` installation is required.
It is recomended to enable [`BuildKit`](https://docs.docker.com/build/buildkit/#getting-started) in docker.

Just run the following inside this directory:

```sh
$> docker build -o . .
```

or

```sh
$> podman build -o . .
```

After some time (it takes aroung 15min in Github CI) a directory named `ffmpeg` will show up with both a `x86_64` and `arm64` directory inside,
both will have a `FFMpeg.framework` for their respective architecture.

### How does the build process work?

The `FFMpeg.framework` is built inside an Alpine Linux container that contains a copy of [`osxcross`](https://github.com/tpoechtrager/osxcross), which is a cross toolchain that enables building native macOS binaries on Linux. Most of the build process is similar to how you would do it in macOS. The main advantage of using `osxcross` is that it handles the configuration for both `x86` and `arm64` and all the required compiling tools without the need for Xcode and with a more direct and easier managment of macOS SDKs. Any required macOS dependencies are handled by a MacPorts-compatible package manager.
