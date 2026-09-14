# The NAS index runbook

The index produced by this run is final: the filesystem gets reorganised
afterwards and nobody is walking these drives twice. So the run has a
pass/fail gate instead of a vibe. Everything here was verified against a
real store on titan (TrueNAS SCALE, 8×12TB raidz2) on 13 Sept 2026.

## What the store captures, verified

Field for field against `stat` on the pool:

- size, mtime (ms), birth time, atime, inode, mode, uid, gid
- symlink targets, verbatim from `readlink`
- hidden flag; every file including hidden, `.git` and dev directories
  when tracked `--unfiltered`
- content identity: full blake3 under 100KB, sampled above; empty files
  and symlinks carry none by design
- unreadable files keep `facet_file.content_error` saying why, so a
  missing identity is always distinguishable from an unattempted one
- non-UTF8 names are recorded lossily and logged loudly; the run report
  should grep the daemon log for `not valid UTF-8`

Not captured, decided: xattrs, Finder tags, resource forks.

## Deploy

Static musl binaries cross-built from the Mac; no Docker, no packages,
runs as an ordinary user:

```
cargo zigbuild --target x86_64-unknown-linux-musl -p sd-cli --release
cargo zigbuild --target x86_64-unknown-linux-musl -p sd-core --bin sd-daemon --release
scp target/x86_64-unknown-linux-musl/release/{sd-cli,sd-daemon} jamie@titan:~/spacedrive/bin/
ssh jamie@titan 'cd ~/spacedrive && (setsid ./bin/sd-daemon --data-dir ~/spacedrive/data >> daemon.log 2>&1 < /dev/null &)'
```

The daemon data dir holds the library and every source store. It is the
artifact of the whole exercise: back it up, and it is what later moves
to the Mac.

## The run, per dataset

Order: calvin-nas and the proxies first (they get deleted), then the
Expansion (its data merges into the pool), then everything else,
jamie-nas last (largest).

```
sd-cli sources track /mnt/pool/<dataset> --unfiltered
sd-cli sources list           # records count settles when the walk is done
# hashing runs behind the walk automatically; watch content rows climb
sd-cli sources freeze <id>    # after hashing lands
```

## The gate, per dataset

1. `sources list` records ≈ `find <root> | wc -l` (find counts the root
   itself, so expect exactly one more).
2. In the store (`sqlite3 sources/<id>/data.db`):
   - `SELECT COUNT(*) FROM record WHERE type='file' AND content_id IS NULL
      AND uuid IN (SELECT record_uuid FROM facet_file WHERE size > 0 AND
      content_error IS NULL)` → 0 when hashing is done.
   - `SELECT COUNT(*) FROM facet_file WHERE content_error IS NOT NULL` →
     read the reasons; permission-denied here is data loss to fix, not
     noise.
3. Spot-check three random rows against `stat`: size, mtime, inode,
   mode, uid, gid.
4. `daemon.log`: zero `not valid UTF-8` lines, or a decision about each.
5. Freeze, then verify the freeze opens read-only and its record count
   matches.

## Before the pool is exported

- One last freeze of every source.
- Save the TrueNAS config backup (System, Save Config). Shares, users
  and service config live on the boot disk, which stays behind; dataset
  properties and ACL data are on the pool and travel with it.
- Confirm no dataset uses ZFS native encryption (`zfs get encryption`).
  If any does, the key or passphrase must travel separately from the
  drives, or the imported pool is ciphertext.
- `zpool status -P`, `zpool get guid`, and `sudo smartctl -i -A` per
  Red, saved as text files beside the data dir. The drive registry
  (`2026-09-13-physical-drives.md` D0/D1) captures what sysfs exposes
  unprivileged; the SMART sweep is the one piece that needs root, and
  the text files are ingestible later through the manual path. Serials
  are unreadable once the drives are crated.
- `zpool export pool`, cleanly. An exported pool imports on a future
  machine without force flags, in any drive order, on any controller:
  the vdev labels on the platters carry the assembly. Any six of the
  eight Reds suffice, which is why all eight ship together.
- Copy `~/spacedrive/data` off the NAS. It is the index; the crate
  should never hold the only copy of the map of the crate.

## Known issues, verified present, not blocking

- **Resumed content jobs can zombie.** A content_identity job
  interrupted by a daemon restart shows Running and does nothing.
  Re-tracking the root dispatches a fresh pass and reuses the same
  source (verified), so the workaround is one command.
- **Sources show "(detached)" while their drive is mounted.** Cosmetic:
  map-only volumes are not persisted, so mount resolution has no row to
  join. Reads, walks, hashing and freezes all work regardless.
- **Graceful shutdown panics** (`TaskHandle done channel dropped`) when
  jobs are running at SIGTERM. The store is WAL-backed and the next
  launch resumes cleanly; the walk re-verifies rather than trusts.
- The volume watcher logs a permission warning for
  `/mnt/pool/ix-applications/docker` every 30s. Noise.

## What the smoke test measured

dev-tools, 28,079 entries, 524MB, on the pool with a CCTV encode
running: walk ~15s, hashing 25,537 files in ~2min (0 unreadable),
freeze 7.9MB in under a second. The pool holds ~270k inodes total
across all datasets, so the full run is bounded by hashing large files'
sampled reads, not by file count.
