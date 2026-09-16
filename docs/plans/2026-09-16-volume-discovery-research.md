# Automatic volume snapshots and explicit sources

Status: read-only audit, September 16, 2026, approximately 03:55–04:05 Pacific.
Code inspected through `0e605105b`. The running Mac daemon reports a build time
of `2026-09-16T10:52:21.950839+00:00`; that timestamp does not prove its exact
revision. No volume was mounted, ejected, indexed, tracked or reconfigured for
this audit.

## Correction to the proposed persistence UX

James clarified that Untitled being untracked is expected. This audit must not
be read as a request to auto-track discovered drives. The agreed
[Add to Library flow](2026-09-16-add-to-library.md) includes a selected scope,
tracks its containing volume, and offers configurable defaults and a choice
of source-store placement. It replaces separate tracking steps in the UX.

A filesystem arena can persist without a registered source. The distinction
between a volume map and a source is not whether any data survives the process.
Do not justify source creation solely as enabling a saved index or remembering
a drive after disconnection. The existing snapshot mechanism already saves
mapped state; loading it and exposing it offline remain separate requirements.

The [cache](../../core/src/ops/indexing/ephemeral/cache.rs) test
`a_tracked_drive_maps_without_appearing_as_a_source` writes and restores a
snapshot with no source registration. Here "tracked" refers to a mapped volume
in the cache, not necessarily a user-tracked library volume.

An explicit source adds a library registration, configured capture scope and
the source record/assertion store. Its own walk retains detail within that
scope rather than using the background map's depth summary. Processing policies
can use its stable target. The UX should describe those actual choices; disk
persistence alone is not a sufficient distinction.

## The running Mac

Read-only `libraries.info`, `core.status`, `core.ephemeral_status`, `volumes.list`
and `paths.context` reported:

- Volume monitoring is running.
- `auto_track_system_volumes` is true.
- `auto_track_external_volumes` is false.
- `Untitled` is mounted at `/Volumes/Untitled`, classified External and untracked.
- Its current path context is `map_state: unseen`, watcher inactive, no source,
  and false for memory, restart cache, source record and source store.
- There is nevertheless a file at its expected volume snapshot path:
  `~/.spacedrive/volumes/53fe8fa3863c51908f6069b8c8b5a76e/ephemeral.snapshot`.
  It is approximately 0.23 MiB, modified September 15 at 03:24:35 Pacific.
  This audit checked the artifact's existence and metadata, not its contents
  or completeness.

The path was derived from the current volume UUID
`b60a4857-ae2b-5c43-8955-df844551f8a1` using `VolumeKey::id` and the
[source directory layout](../../core/src/infra/source_dirs.rs).

[`paths.context`](../../core/src/ops/paths/mod.rs) asks the active cache to
resolve the snapshot path before checking the filesystem. An unmapped volume
can therefore report no restart cache even when the expected file exists.
The earlier September 15 audit saw Untitled mapped and watched. Neither dated
observation establishes what triggered its earlier mapping or the exact time
of a physical reconnection.

## What insertion invokes in the current code

The [volume manager](../../core/src/volume/manager.rs) watches `/Volumes` on
macOS, debounces create/remove notifications by 200 ms and refreshes detection.
A 30-second polling loop is a fallback. New detections emit volume and resource
events. These paths do not call the filesystem mapping operation.

The separate [volume monitor](../../core/src/service/volume_monitor.rs) refreshes
every 30 seconds. Its optional external auto-tracking calls the manager's
`track_volume`, which records volume identity and library tracking metadata.
It does not invoke the public `volumes.track` action that also creates and walks
a source. Enabling that preference therefore does not establish automatic
source indexing through this code path.

There are incidental discovery behaviors: a manager-backed refresh can schedule
a speed test for a newly detected writable visible volume, and tracking can
create/read its Spacedrive identifier. These are not source indexing. The event
and fallback refreshes pass no manager handle, so the speed-test behavior also
depends on which refresh first observes the volume.

## What startup discovery invokes

The [desktop shell](../../packages/interface/src/Shell.tsx) requests
[`indexing.startup`](../../core/src/ops/indexing/startup.rs). With automatic
startup enabled, the daemon claims this once per library per process, restores
known maps, creates a home source if there is no attached source, and calls
[`map_attached_volumes`](../../core/src/ops/volumes/index/map.rs).

That mapping pass visits mounted, visible local volumes, including external
drives. It is not gated by `auto_track_external_volumes`. It registers each
drive in the cache, restores its snapshot when possible, or dispatches a
background walk. A successful restore normally skips a fresh whole-drive walk.
This does not automatically register each drive as a source.

The background map applies
[`Retention::map`](../../core/src/ops/indexing/summary.rs): retain structure
through seven levels and summaries below that. External drives skip the normal
capture exclusions, but still use the depth summary. A saved snapshot contains
what the map retained, not necessarily every deeply nested filename.

A successful [indexing job](../../core/src/ops/indexing/job.rs) saves the mapped
volume snapshot and registers a filesystem watch. Unmapped scratch paths skip
snapshot saving. Ordinary browsing walks the requested directory and does not
itself establish a missing volume mapping or register a source.

## Consequences for the design

- Startup and hot-plug need a consistent discovery/restoration contract.
- Report snapshot artifact existence separately from whether its map is loaded
  or addressable. An unloaded map does not establish absent evidence.
- Distinguish mapped summaries, retained detailed records, source registration
  and ongoing processing when explaining user choices.
- Do not promise complete offline search from snapshot existence alone.
- Keep the agreed policy target separate from `SdPath`; this audit does not
  require locations or a source variant in the addressing primitive.

These findings extend the lifecycle and truthful-state work in
[Source Runtime Reliability](2026-09-15-source-runtime-reliability.md). They do
not settle whether every discovered filesystem should become a source by
default; that remains a product decision.
