# Locations after the entries teardown

Status: separate durable policy target agreed with James; implementation details
remain for review. No runtime code, source, pin, library membership or daemon
configuration changed during this audit.

Inspected September 15, 2026, against `92bb4412e` and the current worktree.
Other development was active in the worktree. Live read-only queries used the
running Mac and titan daemons, which can differ from this checkout.

The agreed addressing boundary is option 1: the policy API accepts `SdPath`,
but persists a separate source-relative target and resolves an execution address
when work runs. `SdPath` gains no source variant. Space items own navigation,
and the location entity is removed without compatibility support. The user
needs selected jobs to keep processing a path's existing and future contents.
Current code facts below remain valid; the direction below replaces the earlier
pin-only, configured-location and source-variant proposals.

James confirmed this version is not backwards compatible. Do not migrate
location rows, preserve location IDs, recover old location job settings, or add
compatibility wrappers for location APIs. Remove the location model and update
current callers directly to Space items, sources and policies.

## Current code and documented direction

Locations survive as named, source-relative navigation pins in the library
database. Sources own ingest, stores, capture policy and watching. A location
does not own an index or records and does not control source retention.

This describes the inspected implementation and existing plans. The revised
proposal below changes that destination; no runtime deletion has been made.

This is the current boundary in [Product Direction](../core/product-direction.mdx)
and [Locations](../core/locations.mdx). The
[locations demotion plan](2026-09-08-locations-demoted.md) explicitly cancels its
earlier proposals to make pins drive retention and watching. The
[final entries drop](2026-09-15-entries-final-drop.md) now removes locations
without a migration. Earlier instructions to preserve the pin model are
superseded by the breaking-release decision above.

| Concept | Durable responsibility | Example |
|---|---|---|
| Source | Origin, ingest, store and source policy | A NAS dataset retained in the library |
| Location | Named source-relative target chosen by the user | Projects inside that dataset |
| Space item | Placement and ordering of a navigation item | Projects in a particular workspace |
| Place | Client projection, with no additional database entity | The sidebar row for Projects, Home or a source |

A source can have many pins and needs none to remain indexed. Known folders
such as Desktop and Downloads are computed for the current device rather than
seeded as location rows. A future offline or protection policy may target a
pin, but must be explicit, independently removable and evaluated by the source.

## What is implemented

The [location entity](../../core/src/infra/db/entities/location.rs) stores a
local integer key plus UUID, source UUID, relative path, name, origin and
creation time. Its table is singular `location`. There are no entry, device or
volume integer foreign keys, scan state, indexing mode or durable totals.
The source and relative path have a unique index.

The [location service](../../core/src/location/mod.rs) already follows the small
model:

- Pinning requires an existing local path inside a registered source. The
  innermost matching source wins. It inserts one row without starting a job.
- Unpinning deletes that row. It does not change source records or watching.
- Renaming changes the label. There is no operation to edit the target path.
- Listing joins the pin to a source root resolved by the runtime registry.
  This path is intended to follow volume remounts.
- Size and file counts come from a loaded arena when available and otherwise
  remain unknown. They are not stored on the pin.

The remaining operation surface is add, list, update, remove, suggested and
validate_path. The [CLI](../../apps/cli/src/domains/location/mod.rs) exposes add,
list and remove. Old indexing and scan controls are not part of the pin action.

Read-only `locations.list` returned `{"locations":[]}` from both running
libraries during this audit:

- Mac: `17bff190-6a4a-42b7-941e-10484357ff92`.
- titan: `8ebcd267-7678-4ef2-9f48-19939379d14f`.

This means neither API returned pins. It does not mean either device has no
Places or Space shortcuts. Listing can also omit unresolved rows, so this API
alone is not a complete count of durable pin claims. The preceding live library
audit separately observed zero location counts.

## Remaining mismatches

These findings identify callers and assumptions to remove or replace. They do
not require repairing or maintaining the retiring location subsystem.

### Source setup still invokes the pin operation

The active [PathBar](../../packages/interface/src/routes/explorer/components/PathBar.tsx)
distinguishes a pin inside a source from adding storage outside one. Its
outside-source branch opens
[AddStorageModal](../../packages/interface/src/routes/explorer/components/AddStorageModal.tsx).
That modal's local submit still calls `locations.add`, which requires an
existing source. This branch cannot create the source its entry point promises.
Inside an existing source, it creates a pin without changing capture.

The cloud branch creates a cloud volume and then passes a Cloud path to
`locations.add`. The backend rejects non-local paths. This can leave the first
step persisted while the second fails. These are code findings; no setup
mutation was executed to reproduce them against the running libraries.

The same modal still asks `locations.validate_path` about indexing risks. The
CLI's interactive location flow also offers cloud paths the pin operation
rejects. The separate old AddLocationModal has no callers in the inspected
tree, but the Add Storage flow is reachable and needs correction.

### Pin sync is declared but does not deliver pins

The entity registers as device-owned even though the row has no owner field.
It declares a `source` dependency, but the source entity has no sync
registration. Its registry table name is still plural `locations`, while the
actual table is singular.

Its `Syncable` implementation returns version zero and inherits the
[trait's](../../core/src/infra/sync/syncable.rs) empty backfill and successful
no-op apply and deletion implementations. Pin, rename and unpin do not publish
sync changes. Turning on library sync will not repair these missing domain
operations. See [Library Sync Research](2026-09-15-library-sync-research.md).

### Resolution assumes a local filesystem source

The row is a useful portable anchor, but the current read path is local:

- `locations.list` resolves against the process-wide ephemeral source registry,
  rather than a complete source view for the selected library. A missing source
  registration causes the pin to be skipped, even though its row survives.
- A registered detached source can still produce a pin with
  `is_available: false`. It is incorrect to say every offline pin disappears.
- [Location::from_row](../../core/src/domain/location.rs) always constructs
  `SdPath::local`. It cannot project the same NAS pin into a correctly qualified
  remote route on the Mac.
- `Location::from_ids`, used for resource projection, passes the database's raw
  source root into that constructor. It does not use the mount resolution that
  listing uses. A volume-relative root can therefore produce a different path.
- Availability reflects the source root's existence, not whether the pinned
  directory exists or its retained records can be read. No pin writer updates
  its relative path when the underlying directory moves or is renamed.
- Pin creation does not use the volume alias normalization used by
  `paths.context`. Equivalent macOS path spellings can resolve differently.

The [sidebar](../../packages/interface/src/components/SpacesSidebar/index.tsx)
disables a pin when `is_available` is false. That blocks navigation even if a
retained source store could answer the listing. Origin access and metadata
availability need the same distinction as source browsing and search.

The [source delete action](../../core/src/ops/sources/delete/action.rs) removes
the store and unregisters the source without handling pins. The pin migration
does not create a database foreign-key cascade. There is no declared pin
lifecycle for that deletion, and an orphan can disappear from listing once its
source is absent from the runtime registry. Location removal retires this
path; the new policy resolver must retain its own unresolved targets.

### The sidebar has two kinds of persisted shortcuts

The PathBar's pin action creates a `location` row. Explorer drag-and-drop in
[DndProvider](../../packages/interface/src/components/DndProvider.tsx) instead
creates `space_item` rows containing `ItemType::Path { sd_path }`. The domain
already also supports `ItemType::Location { location_id }`.

Those paths have different addressing and mutation behavior. Space items can
carry an `entry_uuid` for resolution and explicitly emit sync and resource
events. Location writes do neither. Removing a Space placement and deleting a
location are separate operations, but there is no consistent rule for when a
folder shortcut should use which target.

The [Location Inspector](../../packages/interface/src/components/Inspector/variants/LocationInspector.tsx)
still labels pins Local Storage and displays hardcoded activity and device
lists. It is wired into the Inspector. These tabs are not evidence of actual
device access or indexing history. It also renders an unknown size as zero.

### The historical migration discarded the old locations

[m20260910_000001_locations_are_pins.rs](../../core/src/infra/db/migration/m20260910_000001_locations_are_pins.rs)
drops the old plural `locations` table and creates the singular pin table. It
does not translate old rows. Its comment about recreating default locations is
now stale because known folders are computed Places.

This is historical evidence, not a migration requirement. Do not infer that it
caused a particular missing pin on either running device without historical
evidence. This release does not support migrating either old location rows or
the current pin rows into the replacement model.

## The missing responsibility: continuous processing at a path

The user requirement is: at path X, keep jobs X, Y and Z running as applicable
files appear or change. A navigation-only location does not preserve this
responsibility. Space items already provide bookmarks; source-relative targets
can be supported there without requiring a second navigation entity.

This capability was meant to survive the teardown. T4.4 in the
[execution plan](2026-08-20-entries-teardown-execution.md) says
`location.job_policies` moves to policy rows on source subtrees. The
[original teardown plan](2026-08-20-entries-teardown.md) names `source.db` as
their home. The later capture/navigation/display taxonomy omitted processing.
The replacement is not implemented in the inspected tree.

Historical evidence is available with
`git show 554942ce8^:core/src/domain/location.rs`: `JobPolicies` configures
thumbnails, thumbstrips, proxies, OCR, transcription and object detection. The
old `ops/locations/trigger_job/action.rs` reads those policies for manual
dispatch. That same revision's `ops/indexing/processor.rs` defines watcher
processors, but its configuration loader returns defaults rather than reading
the location. This audit establishes the intended capability and surviving
hooks, not complete historical correctness of every automatic processor.

Current [watcher handling](../../core/src/ops/indexing/handlers/ephemeral.rs)
collects changed source roots and periodically dispatches content identification
when durable pending work exists. The
[job registry](../../core/src/infra/job/registry.rs) discovers job kinds, and
the [manager](../../core/src/infra/job/manager.rs) supports dispatch and active
job deduplication. The current
[thumbnail action](../../core/src/ops/thumbs/generate.rs) runs a selected scope
once. These are useful pieces, but not a general user-configurable processing
policy or a durable subscription to future changes.

## Agreed boundary: SdPath input, durable policy target

Accept `SdPath` when configuring processing at a path. Persist the resolved
source-relative target on the policy. Space items bookmark paths; policies
configure behavior at paths; sources supply ingest and stores. There is no
remaining need for a separate location identity between a path and its policy.
Processing must work without any sidebar item and continue after its bookmark
is removed.

A policy has a stable UUID, a durable target, recursion setting, enabled state
and typed rules. The target is a value containing source UUID and relative path,
not another entity with its own lifecycle. Start with processing rules whose
job kinds define their own settings and eligibility. For example:

```text
Scope: titan / Media / Footage, including subfolders
Maintain: thumbnails, video proxies, transcripts
Execution: the source's designated processing device
```

These job kinds illustrate the product contract; this is not a claim that every
listed processor exists in the current build. Unsupported kinds must remain
visible as unavailable rather than appearing enabled and complete.

Use the source's durable assertion layer for processing intent, following the
original subtree-policy direction. Do not restore `location.job_policies` on
the old table or introduce another entry hierarchy. Exact record/schema and
wire names remain to be designed. The policy ID is sufficient for editing,
sync, progress and removal. Remove the location table, entity, operations,
generated types and sync registration. Update navigation callers to Space items
and source setup callers to source operations. Remove location-specific Space
item variants and inspector code. Implement new processing configuration through
policies. No conversion of location rows, IDs, bookmarks or historical job
settings is required, and old wire methods need no forwarding aliases.

### Keep saved targets separate from execution addresses

The current [SdPath enum](../../core/src/domain/addressing.rs) has Physical,
Cloud, Content and Sidecar variants. Physical stores a device slug and an
absolute path. There is no source-relative variant. Persisting only that
physical spelling would bring remount and device-addressing problems into
both policies and bookmarks.

James selected option 1: keep `SdPath` as the address and store a separate
durable target in the policy/source layer. Do not add `SdPath::Source` or make
the path primitive depend on library source registration.

The conceptual flow is:

```text
Configure: selected SdPath + library context
Persist:   policy.target = { source_id, relative_path }
Execute:   resolve target to the current SdPath for the executing device
```

Recursion remains a policy scope setting. Type and wire names are not yet
specified. This target needs no separate table, identity, registration or
location row. A resolver in the source layer performs both conversions and
checks the selected library's authorization. It must retain an unresolved or
offline target when an execution address cannot currently be produced.

Space items can reuse this target value for durable source bookmarks while
retaining ordinary `SdPath` shortcuts where literal addressing is appropriate.
Sharing a value type and resolver does not couple bookmark removal to policy
removal. Runtime operations continue to receive the addresses they support.

For the first processing implementation, require a registered source and a
scope the job kind supports. A path outside one can lead through source setup
before enabling continuous work. Do not silently create a location or a
separate watcher. Content and Sidecar addresses have no filesystem subtree
semantics; recursive processing must validate the target kind.

Source-relative paths follow source remounts, but do not automatically follow
a directory renamed within the source. Preserve explicit path-target semantics
until evidence-backed rebinding is designed. Unknown or unavailable targets
remain visible. Policy-specific public types must use generated clients; this
decision does not require changing the `SdPath` enum or its serialized variants.

### Reuse the source stream and job executor

Continuous processing should mean:

1. Enabling a rule checks existing records in its scope and queues missing or
   stale work through the existing job executor.
2. Committed source changes mark affected work pending. Filesystem watchers and
   adapter ingests use the same path after updating source records.
3. A small dispatcher coalesces pending work into bounded jobs. Each job domain
   declares supported input kinds, typed settings, prerequisites and how it
   recognizes current output. Keep this modular, like operation registration.
4. Completion corresponds to an input revision and processing configuration.
   If the file changes during processing, work for the new revision remains
   pending. Repeated notifications must not repeatedly regenerate current output.
5. Startup and source reconnection reconcile durable pending state. Events are
   wakeups, not the only evidence that work exists. Idle scopes dispatch nothing.

The source retains one watcher/ingest path. Processing policies select work
from that shared stream; they do not create independent watchers or indexes.
Time-based requirements, such as weekly integrity checks, can use an explicit
schedule. They do not require continuously rerunning every processing job.

For the first implementation, keep one designated executor per source. The Mac
may configure titan's processing rules under the library agreement; replicating
that intent must not launch duplicate jobs on both machines. Processor
availability, local credentials, execution attempts and resource limits remain
facts of the executing device. Unavailable sources retain their rules and
pending work. Distributed execution can follow when there is a concrete need.

For nested configured paths, use the most specific explicit setting per job
kind, with inherit, enabled and disabled states. Different jobs compose; two
matching rules for the same job do not create duplicate work. Expose the
effective settings so the user can understand which rule applies.

The UI can offer a Processing panel on a folder: choose the work, maintain it
automatically, inspect progress, retry failures and disable rules. Adding a
Space shortcut is an independent choice. Disabling processing stops future
dispatch and retains completed output unless deletion is explicitly requested.

## Revised acceptance cases

- A policy created from `SdPath` persists a separate source-relative target.
  Execution resolves that target through the authorized library and source
  layer without adding a source variant to the path primitive.
- Selecting jobs for a subtree processes applicable existing files and newly
  created or changed files, with no navigation pin required.
- Removing or reordering a Space item does not change processing policy.
- Completed output is reused. A file change during a running job is processed
  afterward, and outputs produced by jobs do not cause endless redispatch.
- Restart, missed notifications and source disconnection do not lose pending
  work. Failures remain visible and retry with bounded backoff.
- Nested overrides apply predictably and do not duplicate the same job's work.
- The same target survives remounts and resolves remotely. Switching libraries
  cannot grant access through another library's source registration.
- Configuring a NAS scope from the Mac converges through the library agreement
  and runs only on the selected executor. Unknown job kinds stay unavailable.
- Source setup creates sources; processing configuration and navigation are
  independent follow-up actions. No policy creates another watcher or index.
- The current clients work without location entities, operation registrations,
  generated types or Space item variants. No location migration or compatibility
  layer is introduced. New bookmarks and policies use their own contracts.
