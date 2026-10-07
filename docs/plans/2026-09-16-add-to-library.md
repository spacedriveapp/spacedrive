# Add to Library and source storage

Status: UX direction agreed with James on September 16, 2026. The core half
landed on 2026-10-07 (SPAC-27): library defaults under Adding content with
per-add overrides on `sources.track` and `volumes.track`, store placement
resolved per library and recorded in the registration, volume registration
on every add, the `source.json` descriptor with identity adoption on re-add,
unconditional managed-directory exclusions, and removal that keeps the
catalog unless asked to delete it. Acceptance:
`docs/core/acceptance/add-to-library.md`. The interface half followed
(SPAC-32): one Add to Library modal replaces the storage modal for folders
and whole drives, opened from the Explorer, the path bar, Home and a volume's
menu. It shows the library defaults resolved for the scope, takes per-add
overrides with unavailable placements explained, offers Use these settings
as defaults, shows existing inclusion, and reports what the core saved with
a link to the walk. Library Settings > Adding content edits the defaults.
Offline copies, remount of on-source stores, relocation and consolidation
remain open; the defaults built are the proposals below and remain James's
to change.

## Agreed user flow

Use Add to Library everywhere content can be included: a folder, a whole
volume, a remote path or an adapter entry point. Open one setup modal with the
selected library and scope, configurable defaults, and per-add overrides.
Do not require separate Track Source and Track Volume decisions.

For a filesystem path on a volume, adding it also tracks that volume in the
selected library. Adding the entire volume does the same, with the source scope
at the volume root. Adding a second folder on that volume reuses the volume
registration. Tracking the containing volume does not include all its other
folders as source content.

Untitled being untracked in the September 16 audit is expected. Discovering a
mounted drive or saving an arena snapshot does not express the user's intent
to include it in a library. The
[volume audit](2026-09-16-volume-discovery-research.md) describes current code
paths; their differences do not establish that every plugged-in drive should
be tracked or become a source.

The modal must let the user choose where the source store lives:

- In the library, under the Spacedrive data folder.
- On the source itself, inside a `.spacedrive` directory.

This choice concerns the source's catalog and durable metadata. Adding content
does not move the original files. Library membership remains registered in the
library for both placements.

Remote access is also agreed: the source's Spacedrive instance opens its store
and serves requests or replicates the source through Spacedrive's protocol.
A client reads a replicated database locally; it never opens the remote live
SQLite file through a network mount. Choosing In library or On source changes
store placement on the serving side, not this access contract. For titan's
sources, the Mac continues using Spacedrive's peer connection in either case.

## Proposed modal

The modal opens from the selected path and shows the effective defaults before
the user commits. Use the same flow from Explorer, Storage, the path bar and
source setup. Provider-specific connection steps may precede it.

| Control | Behavior |
|---|---|
| Library | Preselect the active library; identify the destination explicitly. |
| Include | Show the selected folder or whole volume, containing drive and owning device. |
| Name | Prefill a useful name and allow editing. |
| Store catalog | Choose In library or On source; show the actual destination and device. |
| Capture | Show effective inclusion and exclusion settings for the selected scope. |
| Processing | Choose supported ongoing jobs, using the path-policy system. |
| Add to Library | Save the configuration and report initial indexing progress. |

Put defaults under Library Settings > Adding content. Start with In library as
the proposed default placement. The modal may offer Use these settings as
defaults; a one-off override must not silently change future additions.
Defaults initialize new sources and policies. Changing defaults must not move
existing stores or rewrite existing processing rules.

On source should resolve beneath the selected source root. For example, adding
`/Volumes/Archive/Photos` offers a destination beneath
`/Volumes/Archive/Photos/.spacedrive/`; adding Archive offers one beneath
`/Volumes/Archive/.spacedrive/`. The exact internal layout remains to be defined.
Namespace stores by stable identity so libraries and nested scopes cannot
overwrite an unrelated store.

For remote paths, show which device hosts In library storage. Proposed default:
the daemon managing the source holds the primary store. Configuring titan from
the Mac must not silently turn the Mac into the indexing owner. Library sync
controls any copies on the Mac through the library agreement.

Existing inclusion should be visible in the same modal. Re-adding an exact
scope reuses its source. A folder already covered by an entire-volume source
can configure a subtree policy without creating another source. Expanding
several folder sources into an entire-volume source needs a separate design
for preserving identities, assertions and policy targets; do not silently
delete the existing sources to simplify the UI.

## What storage placement means

Both placements use the same source store format, ingest, query contract and
assertion durability rules. Placement selects its home. It does not introduce
a second kind of filesystem source or change library membership.

The portable unit must include committed records, content evidence, assertions,
processing intent stored with the source, and the metadata needed to identify
and reopen it. Calling only a generated index portable would lose user data.
Credentials and device secrets remain outside that portable unit.

The current [directory layout](../../core/src/infra/source_dirs.rs) resolves
source stores beneath the daemon data directory and keeps volume snapshots
and thumbnails separately. This is an implementation constraint to change,
not evidence that the proposed placement control already works.

A volume snapshot can contain siblings outside a selected folder source. Do
not move that whole snapshot into a folder's portable store or advertise it as
a source-scoped export. The agreed
[read routing](2026-09-15-source-runtime-reliability.md) still applies: use a
suitable loaded arena, otherwise read the available source store directly.

Proposed behavior for On source:

- Offer Keep an offline copy in the library, enabled by default for review.
  Explain that it uses additional space to preserve catalog access when the
  source is disconnected. It does not copy the original files.
- Without a full store copy, a saved arena snapshot may still provide offline
  browsing and search at its retained coverage. Report that evidence honestly.
  If neither a suitable map nor a store replica is available, retain the
  registration and report the catalog unavailable. A map is not proof that the
  source's assertions and complete store are retained locally.
- Treat the offline copy as a replica under the same sync and assertion rules,
  not as an independently writable second primary. Offline edits, if enabled,
  require durable assertion delivery and reconciliation.
- Discover an existing portable store and offer to attach it. Its presence
  does not grant library membership or permission to share its contents.

The inclusion of previews, proxies and other large derived artifacts in the
portable unit or offline copy remains a separate retention decision. The modal
must name what its storage choice covers without claiming all caches travel.

## Implementation boundaries

Use one library-scoped add operation behind the UI and CLI. Resolve the
selected address, ensure the containing volume is tracked, register or reuse
the source, persist its effective settings, and schedule initial work. Retries
must reuse the same identities. If setup or job dispatch fails, return the
actual saved state and a retryable failure rather than reporting completed
indexing. Do not invent final wire names before inspecting existing operations.

Resolve storage placement through the source layer instead of constructing
data-directory paths throughout readers and writers. On-source placement must
follow the source's volume anchor across remounts. Library sync can carry the
placement intent; machine-specific absolute storage paths are resolved by the
serving device. Keep the agreed source-relative policy target separate from
`SdPath`. Do not restore locations or add compatibility APIs.

Exclude Spacedrive's own managed directories from ingestion and watcher work,
including for unfiltered sources and enclosing sources. Writing a source store
inside its scope must not index its databases, journals or generated outputs
and continually schedule more work.

Offer On source only when the destination supports the required storage and
write semantics. Explain unavailable choices in the modal. A network mount
must not let several daemons concurrently open one writable database; preserve
the designated writer and serve other devices through the library protocol.
Reconnection cannot create an empty replacement when the existing store is
temporarily unavailable.

Later placement changes belong in source settings and must relocate the
existing store. Flush writes, create and validate a consistent destination,
switch the registration only when it is usable, and preserve a recoverable
copy until cutover succeeds. Follow
[Source Durability](../core/design/source-durability.md); relocation is not a
reindex or permission to discard assertions.

## Acceptance cases

- Add a folder and its containing volume appears once in the selected library;
  only the chosen scope becomes source content.
- Add an entire volume through the same modal and operation with root scope.
- Add another folder on the same volume without another tracking step.
- Every entry point uses Add to Library, the same effective defaults and the
  same per-add overrides. No location operation implements the flow.
- Choose either placement and verify identical source identities, policies and
  query semantics across restart. Show the actual storage device and path.
- Detach and remount an on-source store without losing identity or replacing
  it. Verify offline behavior both with and without a retained library copy.
- Interrupt setup and relocation at their persistence boundaries; retry without
  duplicate registrations, lost assertions or false completion.
- An unfiltered source does not ingest its own `.spacedrive` store or enter a
  watcher loop. An enclosing source also excludes that managed directory.
- Configuring a peer source respects library membership, storage capabilities
  and its designated writer. Finding a portable store grants no access.

The [locations replacement](2026-09-15-locations-research.md) owns processing
policy behavior; [library sync](2026-09-15-library-sync-research.md) owns
membership and replication. This flow composes those contracts.

## Implementation research, September 16

Inspected commits through `4cf25c17c` and concurrent uncommitted work. These
findings are code research, not new live acceptance. No code was changed and
no daemon, source, volume or library was reconfigured. Existing test cases were
read, not rerun. The findings below refine implementation proposals without
making the remaining defaults user decisions.

### Recent work already answers part of the problem

`9e29e7c46` adds `SourceManager::open_read_only` and source-store lookup, listing
and title-search helpers. It opens without schema mutation, store creation,
ledger hydration or a writer. Tests cover relative paths, Unicode matching,
search truncation, refusing writes and refusing to create a missing store.

`4cf25c17c` adds local store fallback to search and detached-directory listing.
This is a first slice of R6: scoped search still attempts snapshot restoration,
directory fallback is currently gated on detachment, and `arena_answers` uses
restoration/indexed-path evidence rather than a complete typed coverage model.
Do not describe it as the completed backend-capability contract or proof that
every cold read now avoids loading an arena.

During this audit, concurrent changes started adding `FetchDatabase`, a
`nested` source flag, `open_file_read_only` and paged store enumeration for
replica hydration. These changes were uncommitted when inspected. Recheck them
before implementation; do not build another database export or arbitrary-path
reader alongside that work. Choosing database delivery for nested sources
addresses sibling leakage. Whole-volume portable catalogs still need the
database's assertions even when an arena is also delivered for fast reads.

The [R7 results](2026-09-15-source-runtime-reliability.md#r7-results-2026-09-16)
supersede the initial broken-fleet observations: the other session recorded all
ten local stores with valid ancestry and all nine titan replicas searchable on
the Mac after a restart with titan stopped. That proof used `4a690c84a`.
The later SQLite commits are repository facts, not proof of the deployed build.
Keep the dated NAS freezes and current replica recovery working through this
change.

### The unified add operation can reuse existing work

The public [volume action](../../core/src/ops/volumes/track/action.rs) calls
`sources::track::track_and_index` for an online drive. The
[source action](../../core/src/ops/sources/track/action.rs) already resolves a
volume anchor and calls `ensure_volume_in_db` for folder additions. Much of the
agreed one-gesture behavior exists beneath the inconsistent UI.

The remaining differences are concrete:

- `ensure_volume_in_db` inserts or refreshes the library volume row. It does
  not call the volume sync publication used by `track_volume`, manage the
  identity file, or emit the same tracking event. Its failure is logged while
  source setup continues. The shared add path needs one reliable volume
  registration contract, without recursively calling the public whole-drive
  action for a folder.
- The volume action defaults external drives to unfiltered capture. Folder
  tracking defaults to filtered capture, and the Explorer Add to Library
  handler explicitly sends `unfiltered: false`. Defaults currently depend on
  the entry point. Resolve effective settings once for both preview and commit.
- `TrackSourceInput.name` is accepted but not passed through execution.
  `set_source_config` and `set_source_name` log persistence errors without
  returning them. The volume action also logs a source-setup failure and can
  still return a successful volume result. The modal cannot promise saved
  settings or successful source setup from these results unchanged.
- [AddStorageModal](../../packages/interface/src/routes/explorer/components/AddStorageModal.tsx)
  still mixes `locations.add`, `volumes.track` and `volumes.index`. The volume
  context menu says Track Volume, while the file context menu says Add to
  Library and skips configuration. Replace these callers with the shared flow.

The [volume manager](../../core/src/volume/manager.rs) writes
`.spacedrive-volume-id`, a separate identity file, not a source store. It does
not conflict with a `.spacedrive` directory. Decide its common tracking behavior
separately from store placement; In library must not be described as a promise
that tracking never writes an identity marker on the drive.

### Placement needs one resolver and a handle lifecycle

The actual database filename in this checkout is `data.db` beneath
`<data>/sources/<source-id>/`. The design name `source.db` is not the current
filename; placement work does not require renaming it.

Both [filesystem opens](../../core/src/ops/indexing/ephemeral/cache.rs) and the
[adapter engine](../../crates/archive/src/engine.rs) use
[`sd_store::SourceManager`](../../crates/store/src/source.rs). That is the
common store boundary to retain. Today its instance holds one parent directory
and appends source ID and filename. Adapters also receive their resolved source
directory as `_data_dir`; changing only filesystem opening would leave adapter
auxiliary files on a different placement contract.

Proposed minimum: the source layer resolves a library-authorized source and
its placement to a concrete store directory and access state. Store code opens
that resolved directory or file. Both ingests use it; neither adds a new store
format or path variant. Placement, source identity and current mount spelling
are separate facts.

Changing the path constructor alone is insufficient:

- `store_for` and `read_store` cache handles by source UUID. A filesystem writer
  also captures its source root when opened. Detach, relocation, remount and
  replica replacement need to retire the relevant handles and reopen against
  the new resolved location. The inspected `detach_library` clears maps and
  registration but not these store-handle maps.
- `SourceStore::open` calls `ensure`, which creates a store. Separate explicit
  first creation from opening an expected existing store. A missing external
  mount or unreadable portable store must not cause an empty replacement.
- `SourceRegistry::remount` exists, but the inspected references are its
  definition and a unit test. The recent startup anchor repair does not prove
  complete live remount handling for movable database handles.
- The filesystem cache has one registry and one database pointer; attaching a
  library replaces them. A library-scoped action alone does not fix that
  process-wide state. Registration and store resolution must take the selected
  library into account, coordinated with the membership plan.

### A portable store needs a small identity descriptor

The filesystem registry reuses `(volume_uuid, relative_root)` in the currently
loaded registry, then generates a new source UUID if no match exists. The
[store schema](../../crates/store/src/record.rs) contains record identities,
relative directory paths, schema metadata and assertion rows. The inspected
filesystem creation path does not store its source UUID, name, volume anchor
and capture configuration as a self-identifying source descriptor in that file.

The existing volume marker identifies a medium, not every source scope on it.
Add a versioned source descriptor inside the portable unit. It needs the source
identity, data type, scope/binding evidence and the information needed to open
the store. Mutable processing intent belongs in the durable assertion layer,
not an independently edited second configuration file. Exact descriptor
storage and publication format remain to be designed.

Attaching an existing store must import its source identity after checking its
binding, rather than invoking new-source registration and minting a replacement
identity. A copied descriptor is evidence to inspect, not automatic authority
to merge two physical volumes or join a library. Device handoff and ambiguous
copies need an explicit binding decision.

### Offline copies can reuse database export

The committed peer inventory records owner/source facts, generation and artifact
paths, and preserves unavailable sources across restart. At the inspected
commit, the artifacts are arena snapshots, not complete source-store replicas.
The database work in progress begins closing that gap.

[`SourceStore::freeze_into`](../../core/src/ops/indexing/ephemeral/store.rs)
already flushes and uses `VACUUM INTO` to create a consistent single-file copy.
Extract or reuse that database snapshot mechanism for portable publication and
local offline copies, while retaining dated freezes as their own user action.
Publish a validated generation and retain the previous valid artifact until
the new one is ready. SQLite documents `VACUUM INTO` as a consistent snapshot;
an interrupted operation can leave incomplete output, so existence is not the
validation gate. [SQLite VACUUM documentation](https://sqlite.org/lang_vacuum.html)

A full source-store copy must retain existing `record_overlay` rows, which
already carry HLC/device fields and deliberately have no cascading foreign key
to generated records. Later tag and policy assertions join that same durability
contract. Local offline copies should use the same replica rules as peer copies.
Initial offline catalog reads do not require concurrent offline editing;
editable assertions require their durable merge and delivery path.

Keep snapshots and volume thumbnail caches in their existing role until an
explicit source-scoped artifact policy replaces it. A folder's portable unit
cannot carry an entire volume map and call its siblings part of that folder.

### On-source writing has two prerequisites

The unconditional own-data check in
[`config`](../../core/src/config/mod.rs) covers one daemon data directory and
its aliases. Discovery, arena writes and watcher dispatch use that check.
The generic `.spacedrive` exclusion in
[`rules`](../../core/src/ops/indexing/rules.rs) belongs to `no_system_files`,
which `RuleToggles::none()` disables. Therefore an unfiltered portable source
needs unconditional protection for its own store before indexing begins.

Extend the existing exclusion boundary to managed source directories, including
ones discovered inside enclosing sources. It must survive remounts, work across
path aliases and cover unregistered portable stores encountered during a walk.
The exact policy for recognizing or reserving `.spacedrive` is still a choice;
it must not rely solely on the active source registry or optional filters.

The store opens in WAL mode. SQLite documents that WAL does not work over a
network filesystem. A mounted writable SMB/NFS folder is therefore insufficient
evidence that the current engine can host its live database there.
[SQLite WAL documentation](https://sqlite.org/wal.html)

James confirmed the remote-access boundary: titan opens its source store and
the Mac uses Spacedrive serving or replication. The WAL restriction does not
introduce another choice in that user flow. It constrains storage that a serving
daemon can itself use for the database. A directly mounted share without a
Spacedrive peer is a separate origin-capability case, not the model for access
to another Spacedrive device's catalog.

### Overlap and removal need defined behavior before cutover

The registry chooses the innermost source for a path, and its test proves that
nested sources share one arena while opening separate stores. That test does
not prove exclusive durable ownership during every walk: a source walk uses
`Retention::source()` with no covered-root exclusions and gives its selected
store to the batch writer. The writer does not re-resolve a source per entry.
An outer and inner source can therefore observe the same subtree into different
stores. Establish nested coverage and ongoing updates before treating a
whole-volume addition as a safe consolidation operation.

For the first UI cutover, reuse already-covered scopes and let policies target
subfolders. Keep an explicit expansion/consolidation result for existing nested
sources instead of deleting them implicitly. This preserves source-relative
policy and bookmark targets while that behavior is designed.

The existing [source deletion action](../../core/src/ops/sources/delete/action.rs)
deletes the source directory through the adapter engine before unregistering
the source. Passing an on-source path through it unchanged would turn a library
removal into destruction of the portable catalog, including assertions.
Separate Remove from Library from deleting the stored catalog. Removing a
folder source also needs an explicit containing-volume retention rule; removing
the last included folder should not silently erase known hardware or stores.

### Suggested implementation order

1. Finish and reuse the current R6 store reader/export work. Preserve the proven
   replica inventory, delivery validation, flush barrier and source identities.
2. Unify library-scoped setup, effective defaults, volume registration and
   truthful persisted results. Route all Add to Library entry points through
   the modal, with processing controls backed by supported policy operations.
3. Add the common placement resolver, portable identity descriptor, expected
   store opening and handle retirement. Extend managed-directory exclusions
   before enabling On source writes.
4. Add on-source placement on supported local filesystems and use database
   exports for optional offline copies. Show origin availability and catalog
   availability separately. Coordinate peer copies with library membership.
5. Enable later relocation, device handoff and overlapping-scope consolidation
   only after their preservation and interruption cases pass. Removal semantics
   must be fixed before the first portable store can be removed from a library.
