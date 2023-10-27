# FFMpeg DLLs for Windows

## Build instructions

To build the FFMpeg `DLLs` a `docker` or `podman` installation is required.
It is recomended to enable [`BuildKit`](https://docs.docker.com/build/buildkit/#getting-started) in docker.

Just run the following inside this directory:

```sh
$> docker build -o . .
```

or

```sh
$> podman build -o . .
```

After some time (it takes aroung 60min in Github CI) a directory named `dlls` will show up with the `DLLs` inside.

### How does the build process work?

This is a modified Dockerfile generate by using https://github.com/BtbN/FFmpeg-Builds
Thanks @BtbN for your great work
