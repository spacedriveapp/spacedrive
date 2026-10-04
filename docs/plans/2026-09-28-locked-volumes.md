# Locked and Unmounted Volumes

> Status: proposed 2026-09-28; nothing built
> Captured: 2026-09-28
> Owns: how a source behaves while its volume is known to the machine but not
> mounted, including an encrypted ZFS dataset whose key is not loaded
> Register: `PROJECT_STATUS.md`
> Companions: `2026-09-15-source-runtime-reliability.md` (R2 anchored roots,
> R5 source health), `2026-09-16-volume-discovery-research.md` (startup and
> hot-plug), `docs/core/design/source-durability.md`

## Outcome

A source whose volume is known but not mounted reads as locked or unmounted,
never as an empty folder. The daemon learns the state from the platform, and
on ZFS from each dataset's `mounted` and `keystatus` properties. While the
volume is away, the source's index serves read-only from its snapshot and
store, and no walk, hash or thumbnail job touches it. When the volume mounts
or unmounts while the daemon runs, the source follows without a restart.

## Where it stands

Read from the code on 2026-09-28, after a TrueNAS machine rebooted with one of
its encrypted datasets still locked. That dataset holds a registered source.

### What the daemon does with a locked dataset

- Linux detection lists volumes from `df`, which leaves out an unmounted
  dataset. `zfs list` runs on every refresh, but only to add detail to volumes
  `df` found, so the locked dataset in its output is ignored.
- `VolumeMonitorService` (`core/src/service/volume_monitor.rs`) updates a
  tracked volume's `is_online` only while detection still returns the volume.
  A volume that stops appearing is logged at debug level and keeps
  `is_online = true`. On the NAS, the locked dataset's row still read
  `is_online = 1`.
- `VolumeIndex::attach_library` resolves anchored roots from the `volumes`
  rows filtered by that flag, so the source resolves to its old mount point.
- ZFS leaves the mount point as an empty directory in the parent dataset, and
  attachment is tested with `Path::exists`: `SourceStatus.attached`, and
  `set_detached` in `attach_library`, `register_source` and `attempt_restore`.
  The source counts as attached.
- The watcher service restores every attached source at startup and arms a
  recursive watch on the empty directory. Loading the key later mounts the
  dataset over that directory. The watch stays on the covered directory, so no
  change inside the dataset is seen until the daemon restarts.
- Nothing re-resolves a source while the daemon runs. The volume manager emits
  `VolumeAdded`, `VolumeRemoved` and `VolumeMountChanged`, the volume index
  subscribes to none of them, and `SourceRegistry::remount` is called only
  from its test.
- Locking a dataset under a running daemon sends `IN_UNMOUNT` and `IN_IGNORED`
  for every watch on it. notify 8.2 forwards neither, so the watch goes silent
  and reports nothing.
- `sources.track` on the empty mount point resolves it to the parent volume,
  which is mounted, and registers a second source anchored there.

The source shows as online. Its listings come from the snapshot, every read
fails, and once the key loads it misses changes until a restart.

### What keeps the index intact

None of these knows the volume is locked. They hold through ordering and
thresholds:

1. The restore runs first. The watcher service restores the source's map from
   its snapshot at startup, and the startup pass runs `restore_everything`
   before the coverage heal. A browse of an indexed directory answers from the
   map and never reads the disk.
2. The coverage heal walks a source only when its store holds records and its
   map has no children at the root. A restored map has children.
3. Only a walk that enumerates a whole volume with no rules opens a sweep
   (`IndexerJobConfig::enumerates_whole_source`). Both the heal and
   `sources.track` decide that from the volumes detection found, so a walk of
   the empty mount point runs under the parent volume and cannot sweep.
4. The store refuses a sweep that would remove more than half of its records
   once it holds more than 100 (`finish_sweep` in `crates/store/src/file.rs`).
5. `save_snapshot` refuses a map more than ten times smaller than the last
   save, or than the source's record count before the first save, from 1,000
   entries up.
6. Hashing a file that cannot be read records the failure and takes the file
   out of the pending set. The record stays.

Starting a daemon while a dataset is locked therefore keeps the index. The
last guard has a cost: a file that was pending identification while its
volume was away is not retried when the volume returns.

## Design

### A known volume is mounted, unmounted or locked

Detection reports one of three states for every volume a library tracks:

- **Mounted.** It is in the mount table at its mount point.
- **Unmounted.** It is known and missing from the mount table.
- **Locked.** It is unmounted because its encryption key is not loaded.

On ZFS, the `zfs list` call that already runs on each refresh adds `mounted`,
`encryption` and `keystatus`. A dataset with a mount point, `mounted` set to
`no` and `keystatus` set to `unavailable` is locked. `zfs list` needs no
privileges, so no TrueNAS middleware call is involved. A tracked volume
matches its dataset by mount point, since nothing inside a locked dataset,
including an identity file, can be read. Other filesystems report mounted or
unmounted. Encrypted APFS volumes and LUKS devices can report locked through
the same state later.

The state is live and derived on each refresh, so it needs no new column in
`library.db`.

### A volume that disappears goes offline

The volume monitor marks a tracked volume offline when detection stops
returning it, as it already does when a detected volume unmounts. This fixes
the stale flag on every filesystem.

`attach_library` resolves roots against the volume manager's live state
instead of the stored flag. Detection finishes in `Core::new_with_config`
before any library opens, so the live state is ready at attach. With volume
monitoring disabled, the stored flag stays the fallback.

### Attached means mounted

A volume-anchored source is attached when its volume is mounted.
`SourceStatus.attached`, `set_detached` and `attempt_restore` read the
volume's state instead of testing the root with `Path::exists`.

Before any walk of an anchored source, dispatch checks that the volume's mount
point is a mount point: on Unix, its device differs from its parent
directory's, or it is the filesystem root. On the NAS, the locked dataset's
mount point had the same device as the pool root, and a mounted sibling had
its own. The check holds whatever the stored state says, so an empty mount
point is never walked as a source. `sources.track` refuses the mount point of
a known volume that is not mounted, and names the volume's state.

A source that is not attached keeps what it has. Its map restores read-only
from the snapshot, listings fall back to the store, and a read fails with the
volume's state as the reason. Hashing, thumbnails and the coverage heal skip
it.

### Following mounts while the daemon runs

The volume index subscribes to the volume manager's events.

- **Unmount or lock.** The source detaches on the next refresh. Its watch is
  dropped, its map stays readable, and nothing new is dispatched at it. Every
  guard above still holds during the refresh interval.
- **Mount, including a dataset whose key just loaded.**
  `SourceRegistry::remount` resolves the root at the current mount point and
  persists it. The map restores if it has not, the
  watch is armed on the mounted filesystem, and files that failed
  identification while the volume was away return to the pending set.

This is the hot-plug half of the startup and hot-plug contract the September
16 volume audit asks for.

### Showing it

`volumes.list`, `sources.list` and the volume facts a device publishes to its
peers carry the state. The app and CLI show a source as locked or unmounted
with its volume's name, and a paired device shows the owner's volume as
locked instead of a replica that stopped updating. This is the origin
availability fact in R5's source health. If R5 lands first, the state is one
of its fields.

## Phases

| Phase | Scope | Exit proof |
|---|---|---|
| L1 | A volume that disappears goes offline, and `attach_library` resolves against live detection | A source whose volume is gone at startup comes up detached, with its map restored read-only, no watch armed, and `sources.list` showing it offline. A test covers the monitor marking a tracked volume offline when detection stops returning it |
| L2 | Attached means mounted, and the mount point check runs before any walk | With a volume's mount point left as an empty directory, no walk, hash or thumbnail job runs over its source, a forced heal refuses with the reason, and `sources.track` on the mount point refuses instead of registering a second source |
| L3 | ZFS dataset state | On a pool with an encrypted dataset whose key is not loaded, `volumes.list` reports it locked and `sources.list` reports its source locked. After the key loads, both report mounted within one refresh |
| L4 | Following mounts while running | Lock a dataset under a running daemon, then load its key again. The source detaches within one refresh and loses no records, and once the dataset mounts, a file created inside it reaches the store through the watcher, with no restart |
| L5 | Showing it | The app and CLI show locked and unmounted volumes and sources, on the owner and on a paired device |

L1 and L2 apply to every filesystem and come first. L3 adds what ZFS reports.
L4 needs L1 and L2. L5 goes last, alongside R5 if that is underway.

## Acceptance

Run L1 and L2 on Linux against a loop-mounted image, unmounted with its mount
point left in place. Run L3 and L4 on a file-backed ZFS pool with an encrypted
dataset, then once on the NAS.

## Until this lands

- Starting the daemon while a dataset is locked keeps the index. Restart the
  daemon after loading the key, so the watch lands on the mounted dataset.
- Stop the daemon before locking a dataset it has files open in. The lock
  refuses a busy dataset, and forcing it removes files from under the daemon.

## Decisions for James

1. What a locked source shows. Proposed: its names, folders and thumbnails
   stay browsable from the snapshot and store, labeled locked, the way a
   detached drive's do. The alternative hides them on this device and its
   peers until the key loads. Locking a dataset leaves its index readable: the
   store, snapshot and thumbnail sidecars live in the data directory, and
   paired devices hold replicas. If a dataset is locked to protect its
   contents, its file names and thumbnails stay visible through Spacedrive
   unless the index is hidden as well.
