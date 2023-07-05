# macOS cross toolchain configured inside Alpine Linux

This container based on alpine 3.17, with the most common build decencies installed, and a built version of [`osxcross`](https://github.com/tpoechtrager/osxcross) plus the macOS SDK 12.3 (Monterey) targeting a minimum compatibility of macOS 10.15 (Catalina) for x86_64 and macOS 11.0 (BigSur) for arm64.

**Image Tag**: macOS SDK version + osxcross commit hash + revision

This container is currently available at:
https://hub.docker.com/r/vvasconcellos/osxcross.
