# AppImage build script and files

This directory contains the script and recipe to build an AppImage from a Spacedrive `.deb` using [appimage-builder](https://appimage-builder.readthedocs.io/en/latest/index.html).

## Instructions (Requires a Linux environment)

- Install one of the following container runtimes:

  - [Podman](https://podman.io/docs/installation#installing-on-linux)

  - [Docker](https://docs.docker.com/engine/install/#supported-platforms)

- Set up your development environment following the steps in the [CONTRIBUTING](../../CONTRIBUTING.md) guide

- Build a production release of Spacedrive by invoking `pnpm tauri` in a terminal window inside the Spacedrive repository root

  > After the build finishes you should end up with a `.deb` archive in `target/release/bundle/deb`

- Change your current work directory to `scripts/appimage`

- Execute the `build_appimage.sh` script inside a `debian:bookworm` container

  - Podman: `podman run --rm -v "$(CDPATH='' cd ../.. && pwd -P):/srv" -w /srv debian:bookworm scripts/appimage/build_appimage.sh`

    - You may have to run Podman with `podman run --privileged` if you get a permission denied error

  - Docker: `docker run --rm -v "$(CDPATH='' cd ../.. && pwd -P):/srv" -w /srv debian:bookworm scripts/appimage/build_appimage.sh`

    > If you are running a system with selinux enforcing you will need mount the `/srv` volume with the `Z` flag to avoid `Permission denied` errors. [more info](https://docs.podman.io/en/latest/markdown/podman-run.1.html#volume-v-source-volume-host-dir-container-dir-options)

  > After the script finishes you should end up with an `.AppImage` executable in `target/release/bundle/appimage`
