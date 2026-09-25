# Opening Files on Another Device

> Status: O1, O2 for the desktop app, O4 on macOS and O6 landed 2026-09-24;
> O3, O5 and the web client's route are open.
> Captured: 2026-09-21
> Owns: opening and previewing files whose bytes live on another device
> Register: `PROJECT_STATUS.md`
> Companions: `docs/core/design/mounts.md` (byte plane, block cache, mount
> frontends, pinning), `2026-08-20-byte-plane-and-block-cache.md`,
> `2026-09-19-incremental-replication.md` (tiles and sidecars for replicas)

## Outcome

A file listed from another device opens the way a local one does. Spacedrive's
own preview streams it and reads only what it shows. Another app gets the file
one of two ways, chosen by a single on/off setting: streamed through the
mounted share, or copied to a temporary file first. The Photos window lists
another device's folder and previews it in Quick Look through the mounted
share. Nothing ever opens a local file that happens to share a remote file's
path.

## Where it stands

Measured on the Mac against titan on 2026-09-21.

The byte plane works. The Mac's daemon already serves every titan source over
loopback HTTP, backed by the byte plane and the block cache:

```
GET http://127.0.0.1:7764/dav/jamie-nas%40titan-cc1448/Ingest/Pic/SA701868.JPG
Range: bytes=0-1048575

206 Partial Content
accept-ranges: bytes
content-type: image/jpeg
content-range: bytes 0-1048575/30133153
```

The whole 30 MB file took 6.3 s cold over Tailscale, and 0.04 to 0.12 s on
repeat reads from the block cache. The cache holds 1 MB blocks: 128 MB in
memory (L1), and up to 8 GB on disk (L2) under `sources/<source-id>/blocks/`,
capped by `mounts.cache_max_bytes`.

The same sources are served over SMB on loopback. Nothing mounts it:
`mounts.status` prints a `mount_smbfs` command for a person to run.

The app uses none of this:

- **Open**, from the explorer context menu or a list row, passes
  `sd_path.Physical.path` to the Tauri opener without checking which device
  the path is on. For a titan file it asks macOS to open `/mnt/pool/...`,
  which fails. If the same path exists on the Mac, it opens the Mac's file.
- **QuickPreview** and the mesh viewer load originals through Tauri's asset
  protocol with the same unchecked path, so they fail the same two ways.
- The web client has no route for original bytes, so its preview never loads
  an original, local or remote.

What landed on 2026-09-24, verified on two paired throwaway daemons on one
Mac, where the owner's paths also exist on the reading side:

- `files.stream_url` and `files.local_path` are core queries over one
  resolver, `mounts::share_path`, which turns an `SdPath` into its place in
  the share: beneath the innermost local source, or beneath the replica
  holding the path on its owner. A remote clip streams through the URL with
  206 ranges and its content type, and its bytes match the original.
- `files.local_path` mounts the SMB share at `<data-dir>/mount` through
  `NetFSMountURLSync`, read-only, soft and hidden from Finder, in about
  0.3 s. `Core::shutdown` unmounts it; startup unmounts one a killed daemon
  left behind, before volume detection lists mounts; volume detection skips
  mounts inside the data dir; `core.reset` unmounts before it empties the
  data dir. Quick Look makes a thumbnail of a 60 MB remote clip through the
  mount in 0.3 s, fetching 5 MiB of it.
- The desktop app's preview reads every original through one hook,
  `useOriginalUrl`: the asset protocol for a file on this device, the
  stream URL for one on another. The CSP allows media from loopback. Open and
  double-click resolve a remote file through `files.local_path`; Open With,
  Show in Finder and Share appear only for files on this device.
- Photos follows a folder on another device. `search.media` pages replicas
  after local stores, from the replica's index, since most replicas arrive
  as snapshots with no store beside them. Cells keep their `SdPath`, so
  thumbnail requests go to the owner, and Quick Look on a remote cell asks
  the data plane for its path, which answers in process for a local file.
  Only the latest answer reaches the panel.
- The SMB frontend had two defects that made an app read the wrong file.
  Every entry reported file id 0, and `list_dir` ignored the search pattern,
  so a client looking one name up got the directory's first entry back as
  that name's. After a listing, macOS served `clip.mp4` with the size and
  bytes of `tail-moov.mov`. Entries now carry a hash of their share path as
  their id, and a name lookup answers with that entry alone.
- The HTTP share refuses a request whose Host is not loopback, which stops a
  web page that points its own domain at 127.0.0.1, and answers CORS only for
  Tauri web views and pages served from loopback.

## Design

### Who reads decides the path

**Spacedrive's own viewers always stream.** A preview never needs the whole
file, so there is no setting for it. A photo shows after its first ranges
arrive, and scrubbing a video reads the ranges played.

**Other apps need a file path**, and there are two ways to give them one.
Setting 1 picks between them.

### Setting 1: download remote files before opening

One on/off switch per device, **off by default**. There is no size threshold:
a device either streams or copies.

- **Off, stream.** Spacedrive attaches its share as a mount and hands the app
  the file's path inside it. The app reads only the ranges it touches, through
  the block cache.
- **On, copy.** The daemon copies the whole file to a temporary folder and
  opens the copy. Every app works with it, including ones that misbehave on
  network volumes or write files next to the one they open.

Off is the default because the files this matters most for are the largest.
Titan's footage has single files of 70 GB, more than the Mac's free disk, and
streaming is the only way to open those at all.

### Setting 2: cache streamed files on disk

One on/off switch per device, **on by default**.

- **On.** Streamed blocks land in memory and on disk as they do now, within
  the existing cap.
- **Off.** Memory only. No original bytes read from another device are
  written to this disk, except a copy asked for through setting 1. Thumbnails
  and their sidecars are separate and keep their own storage.

Either way, a full disk never fails a read. A block the disk refuses stays in
memory.

The two settings are independent. Streaming with setting 2 off is the
"in memory only" mode; setting 1 on is a temporary download.

### Streaming inside Spacedrive

**Desktop app.** For a remote file, the viewer's `src` is the daemon's share
URL for it. The share already answers ranges and content types and binds to
loopback. A daemon query, `files.stream_url { path: SdPath }`, returns that
URL, so clients never build share names themselves. Local files keep the
asset protocol, which reads the disk directly.

**Web client.** The browser cannot reach the daemon's loopback share, and it
cannot read disks at all. `sd-server` gains a route that forwards ranged
`GET`s to the daemon's share, behind the same basic auth as the rest of the
server. The web preview reads every original, local or remote, through it.

### Opening in another app

**Stream.** The daemon attaches the SMB loopback share the first time an open
needs it, at a mount point under its data directory (`NetFSMountURLSync` on
macOS, which needs no admin rights for a directory the user owns), and
detaches it at shutdown. A core query, `files.local_path { path: SdPath }`,
returns the file's path inside the mount, and the app hands that to the OS
opener. It is a query although the first call mounts: the mount changes
nothing in the library, an action writes two audit rows and a sync entry per
call, and Photos asks for a path each time the cursor moves while Quick Look
is open. When the FSKit module lands (mounts phase 4), it replaces SMB
underneath without changing this interface. Until a platform has a way to
attach a mount, Open copies there whatever setting 1 says, and says why.

**Copy.** `files.open_copy { path: SdPath }` runs a job that fetches the file
through the byte plane into `<data-dir>/opened/<device>/<source>/<path>`, then
returns the local path.

- The copy keeps the file's name and extension, so the opening app and its
  title bar show the real file.
- A second open of an unchanged file reuses the copy. The replica's size and
  modification time are the version, as they are for blocks.
- Before any bytes move, the daemon compares the file's size with the free
  space where the copy would go, and refuses with that reason instead of
  filling the disk.
- The copy is read-only. Edits never reach the owner in this design, so a
  writable copy would invite work that silently stays behind. Read-only makes
  the app offer Save As instead.
- Copies are temporary. The daemon clears `opened/` when it starts. On macOS
  and Linux a copy still open in an app keeps working, since removing an open
  file does not affect the app reading it. Windows refuses to remove an open
  file, so that copy goes at the next start instead.

**Local files** open exactly as they do now.

### Telling local from remote

Every `File` already carries its device in `sd_path` and an `is_local` flag.
The client routes on them: a local file goes to the OS opener or the asset
protocol, and a remote one goes through the daemon. No path from another
device reaches the local opener, whatever setting 1 says.

### When the owner is offline

- **Stream:** ranges already in the block cache still serve. Anything else
  fails with the byte plane's readable status, which names the device.
- **Copy:** cannot start, and says so.

Pinning (mounts phase 5) is the answer for files needed offline.

### Out of scope

- Writing back to the owner.
- Renditions and proxies (mounts design).
- Pinning (mounts phase 5).
- Drop takeover (mounts phase 3).

## Phases

| Phase | Scope | Exit proof |
|---|---|---|
| O1 | Open and previews check the device | Landed 2026-09-24. Preview, Open, Open With, Share and the inspector's Share route on `is_local`; a remote file reaches no local opener or asset protocol path. In the two-daemon test, where the owner's paths also exist on the reader, a remote file was read through the peer protocol, which `mounts.cache_status` counted, and never from the local copy. Show in Finder on a sidebar item still takes its path unchecked, since the interface has no slug comparison for a bare `SdPath` |
| O2 | Streaming inside Spacedrive | Desktop landed 2026-09-24: previews stream remote originals from `files.stream_url`, verified byte-exact with ranges between two paired daemons; the scrub measurement against titan is open. The web client's `sd-server` route is not built. The cache module doc names the real L2 location |
| O3 | Open by copy | With setting 1 on, opening a titan file runs a copy job and opens a read-only copy with the file's own name. A second open reuses it. A file larger than the free space is refused before any bytes move. `opened/` is empty after a daemon restart |
| O4 | Open by stream, macOS first | Landed 2026-09-24 with no setting, since O3 and O5 are open: opening a remote file launches its default app through the share `files.local_path` mounts. The mount detaches at shutdown and a killed daemon's mount is removed at the next start. A titan clip larger than the Mac's free disk playing in QuickTime is open |
| O5 | The two settings | Both switches appear in settings and in the daemon's config. With setting 2 off, streaming a titan file writes nothing under `sources/*/blocks/` |
| O6 | Photos previews another device's files | Landed 2026-09-24. Photos follows a folder on another device, pages its media from the replica, requests its tiles from the owner, and shows a cell in Quick Look through the mounted share. Scrubbing a titan video in Quick Look is open |

O1 goes first because it is a correctness bug. O2 comes next because
previewing is the common case and needs no mount. O3 comes before O4 because a
copy works on every platform and needs nothing attached. O4 and O6 landed
ahead of O3 because Quick Look reads only by path, and streaming a large video
into it needs the mount.

## Acceptance

Measure on the Mac against titan over Tailscale, with the block cache cold and
then warm: time to first byte of a preview, bytes fetched for a one-minute
scrub, time for a copy of a known file, and the files under
`sources/*/blocks/` and `opened/` after each run.

## Decisions taken

Loopback access (formerly decision 2). The share stays open to processes on
this machine. Pairing already authenticates the hop between devices: the
byterange protocol answers only paired devices. The loopback hop is between
processes on one machine, and the daemon's RPC port on `127.0.0.1:6969` is
open to all of them too, so a share key would stop nothing that RPC does not
already allow, since any local process could ask RPC for it. The one reach the
share adds is HTTP, which a web page can get at by pointing its own domain at
127.0.0.1; the share refuses any request whose Host is not loopback, and
answers CORS only for Tauri web views and loopback pages. Keeping other
local accounts and containers out is daemon-wide work, such as serving RPC
over a Unix socket only its owner can open.

## Decisions for James

1. The default for setting 1. Proposed off, stream, for the reason above.
2. Where copies live and when they go. Proposed `<data-dir>/opened/`, cleared
   at daemon start. The alternative is the OS temporary directory, which the
   OS clears on its own schedule.
3. The disk cache cap is a fixed 8 GB, which is most of this Mac's current
   free space. Expose it next to setting 2 so it can be sized per device?
