# External Tools

> Status: first provider implemented

Spacedrive keeps large native dependencies out of its default distribution. An
external tool is software installed on the same machine that supplies an
optional capability through a reviewed process boundary. FFmpeg is the first
provider. It restores video thumbnails and thumbstrips without linking or
shipping FFmpeg in the default binary.

## Contract

`ExternalTools` is machine-scoped and lives in `CoreContext`. A registered tool
has a stable ID, discovered executables, a version, capabilities, and supported
install options. `tools.list` exposes that state to every client. A missing
tool removes only the capabilities it supplies.

You can inspect the same operation through the generic CLI surface:

```bash
cargo run --bin sd-cli -- op tools.list
```

Discovery checks, in order:

1. An explicit Spacedrive environment override.
2. The daemon's `PATH`.
3. Platform package-manager locations such as `/opt/homebrew/bin`.

The third step matters for desktop launches. macOS applications do not inherit
the interactive shell's full `PATH`, even when Homebrew installed the tool for
the same person. Missing tools are rechecked on use, so an installation becomes
available without restarting the daemon.

## Installation

`tools.install` accepts a registered tool, a supported installer, and
`confirm: true`. It rejects calls without the explicit confirmation bit. The
service runs only fixed recipes:

- Homebrew installs `ffmpeg` on macOS.
- WinGet installs `Gyan.FFmpeg` on Windows.

For example, an explicit Homebrew request is:

```bash
cargo run --bin sd-cli -- op tools.install --json '{"tool":"ffmpeg","installer":"homebrew","confirm":true}'
```

Linux discovery supports normal system locations. Automatic Linux installation
is intentionally absent because apt, dnf, and pacman usually cross a privilege
boundary. A client can show the appropriate system instructions without asking
the daemon to obtain root access.

Package-manager installs are host-managed software. Spacedrive reports their
path and version but does not claim their bytes match a Spacedrive-controlled
hash. A future managed-download provider must pin a version and verify its hash
before publishing the executable to this registry.

## Execution boundary

Callers select a registered executable and pass an argument vector. The runner
never invokes a shell. Standard input is closed, output is captured in bounded
temporary files, and media operations have execution deadlines. This prevents
a malformed file from holding a worker forever or producing unbounded captured
output.

The first FFmpeg consumers are:

- the thumbnail producer chain, which asks the host executable for one PNG
  frame and stores its BGRA pixels in `thumbs.pvcache`;
- the thumbstrip worker, which uses FFprobe duration evidence and generates a
  5 by 5 PNG sprite across the full timeline.

Thumbstrips are requested on hover or timeline interaction. They live at
`volumes/<volume-id>/thumbstrips/<record-id>/<version>.png`. The record version
comes from file size and full-precision modification time, so a changed video
gets a new immutable URL and artifact.

## Linked feature

The `ffmpeg` Cargo feature remains available for code that needs in-process
FFmpeg libraries. It is not required for host-tool discovery, video thumbnails,
or thumbstrips. Default builds remain small and can use the person's existing
FFmpeg installation.
