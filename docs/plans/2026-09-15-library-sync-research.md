# Library sync, membership and source ownership

Status: research and proposed direction for review. No library was joined,
merged, reconfigured or restarted during this research.

Code inspected at `adf729cd9`, including existing uncommitted work. Live
observations were captured on September 15, 2026, at approximately 20:45–20:49
Pacific, September 16 at 03:45–03:49 UTC. The deployed binaries differ from
this checkout; live observations and code findings are identified separately.

## Finding

The Mac and titan have a trusted network connection but belong to different
libraries. Source replication crosses that boundary automatically. The
overview combines peer storage summaries with the selected library's device
count, so the user sees NAS storage under a library that contains only the Mac.

Library sync still has a job after entries disappear. It should establish and
retain library membership, source registrations, volume knowledge, grants and
shared organization. Source generations and file bytes keep separate transfer
paths, authorized by that library agreement.

This direction already appears in [Library Sync](../core/library-sync.mdx),
[Library](../core/library.mdx) and [Devices](../core/devices.mdx). Those documents
describe the intended boundary more consistently than the current code enforces
it. They cannot be used as proof that membership and grants work today.

## What the running instances actually report

| Observation | Mac | titan |
|---|---|---|
| Library name | My Library | My Library |
| Library UUID | `17bff190-6a4a-42b7-941e-10484357ff92` | `8ebcd267-7678-4ef2-9f48-19939379d14f` |
| Device UUID | `a688cd39-b65a-4392-9767-7a08aa8fd68f` | `3cee5a0b-d634-425d-89c1-3e8fde2511f6` |
| Library device rows | Mac only | titan only |
| Eligible library sync partners | 0 | 0 |
| Tracked volumes returned by this library | 1 | 9 |
| Sources returned, including peer replicas | 9 | 10 |
| Library setting `sync_enabled` | false | false |
| Local device row `sync_enabled` | true | true |
| Sync metrics state | Uninitialized, no recorded transfers | Uninitialized, no recorded transfers |
| Binary build time, UTC | `2026-09-16T01:46:44.804978` | `2026-09-15T23:05:47.319778` |

The Mac's dedicated network status reports one paired peer and one connected
peer. Its nine listed sources comprise its home source and eight titan replicas.
Titan's ten comprise nine owned sources and the Mac replica. Source listings
are therefore already broader than library membership.

The Mac's library info returned 269,019 files and about 67 TB of logical file
bytes at one observation, alongside `device_count: 1`. These are changing
summary values, not a fresh recount or a physical-capacity guarantee.

The queries used were `network status`, `library list`, `library info`,
`devices list`, and the read operations `sync.partners`, `sync.metrics`,
`volumes.list` and `sources.list`. Remote operations used titan's actual library
UUID. A sanitized structured capture is retained locally at
`/private/tmp/spacedrive-library-sync-research-2026-09-15.json`.

The ordinary `sd-cli --device titan sync partners` command sent the Mac's
library UUID and failed with `Library not found`. Calling the operation with
titan's explicit library UUID succeeded. Some CLI domains resolve the remote
library correctly while this sync command still carries the local context.

## Why the current behaviour occurs

### Pairing persists network trust, not library membership

[`DeviceRegistry::complete_pairing`](../../core/src/service/network/device/registry.rs)
records the peer, node mapping, slug and pairing persistence. It does not
insert that peer into a selected library's `devices` table.

That separation is useful internally. A physical runtime can participate in
more than one library, and trusting its network identity need not grant every
library on the machine. The user flow currently leaves this internal step
looking like the entire agreement.

### Source replication is attached to device connectivity

[`mounts::watch_peers`](../../core/src/service/mounts/mod.rs) starts source
replication when a paired device connects. Its refresh loop also operates by
device. Neither starts from a library's membership and source subscriptions.

[`ByteRangeRequest`](../../core/src/service/network/protocol/byterange.rs)
contains `ListSources`, `FetchSnapshot { source_id }` and `DeviceSummary`
without a library identifier. The server lists the global filesystem cache's
sources. `DeviceSummary` selects the first open library. Admission checks a
known node mapping; path reads check membership beneath an attached source
root. These paths do not express the documented library or source grant.

[`RemoteShare` and `DEVICE_SUMMARIES`](../../core/src/service/mounts/peer.rs)
are process-global maps keyed by source or device UUID, with no library scope.
[`sources.list`](../../core/src/ops/sources/list/query.rs) reads its selected
library's source rows and then appends all remote shares.

The serving device still needs to transfer metadata and bytes. The missing
piece is the library's authority over which device may serve which registered
source to which recipient.

### Overview fields describe different sets

[`Library::calculate_device_count_static`](../../core/src/library/mod.rs)
counts rows in the selected library's `devices` table.
[`libraries.info`](../../core/src/ops/libraries/info/query.rs) subsequently
adds all cached device summaries to file, content and capacity totals through
`add_device_summaries`. That helper does not add library members or filter by
library. The [overview](../../packages/interface/src/routes/overview/index.tsx)
renders the resulting fields together.

Changing the device count to the number of paired peers would conceal the
missing membership. Once source registrations replicate, retaining the current
summary addition could also count the same source on multiple devices.
Library totals need one defined source set, with replicas counted separately
from distinct source records and physical capacity deduplicated appropriately.

### The disabled setting does not describe the runtime

[`LibrarySettings`](../../core/src/library/config.rs) defaults `sync_enabled`
to false. The [configuration update](../../core/src/ops/config/library/update.rs)
persists that value but does not start or stop the service.

The inspected startup paths in [`core/src/lib.rs`](../../core/src/lib.rs),
[`LibraryManager`](../../core/src/library/manager.rs) and
[`Library::init_sync_service`](../../core/src/library/mod.rs) initialize and
start sync without checking that setting. Live `sync.partners` and
`sync.metrics` succeed, which requires an existing sync-service object.
The metrics label `Uninitialized` is a sync progress state, not proof that the
service does not exist.

[`get_connected_sync_partners`](../../core/src/service/network/transports/sync.rs)
selects other library device rows with their own `sync_enabled: true` and a
paired node mapping. Each current library has only its own row, so neither has
an eligible peer. Flipping the library setting would not establish membership.

### Share and join exist, but completion is unreliable

[`network.sync_setup`](../../core/src/ops/network/sync_setup/action.rs) exposes
sharing a local library, joining a remote library and merging libraries.
Joining creates a local library with the remote UUID. Merging returns an
unimplemented error.

The share and join paths send `RegisterDeviceRequest`, but failed registration
responses and transport errors can be logged before returning
`success: true, devices_registered: true`. The local registration helper is
defined but has no call sites in this file.

The [message handler](../../core/src/service/network/protocol/messaging.rs)
attempts registration in the reverse direction through a spawned task after a
new device insert. Updating an existing device does not perform that reverse
exchange. This leaves a retry unable to prove both sides converged. A request
with no library ID can target every open library, and the library handler
does not use its authenticated `_from_device` argument to authorize the
requested membership change.

These are code findings, not faults injected into the user's live libraries.
A repaired join must durably track partial completion and retry it; a remote
acknowledgement cannot be replaced by a success-shaped local response.

### Library isolation is incomplete below the operations

[`EphemeralIndexCache`](../../core/src/ops/indexing/ephemeral/cache.rs) holds
one registry and one optional library database handle. `attach_library`
replaces them; `detach_library` clears them. Adopting source and volume rows
also needs to distinguish observations made on this device from remote facts.
Simply replicating a NAS mount point must not make it a local path to watch.

[`remote_ops`](../../core/src/service/network/protocol/remote_ops.rs) carries
an optional library UUID, but defaults to the first open library when omitted.
Its dispatch path admits paired devices and invokes the ordinary operation
executor without passing a library-scoped remote principal into that call.

Library-aware sync must cover these consumers too. Multiple libraries need
independent membership, source projections and authorization, even if they
share a machine-level volume arena or a physical cached artifact.

## Which structures survive entries

| Data | Current implementation | Required ownership and sync treatment |
|---|---|---|
| Device network identity | Machine registry and pairing persistence | Machine-local keys and authenticated peer identity; library membership refers to it |
| Library membership | Presence in `devices`, plus a sync flag; old shared-row registration | Durable member state, role/grants and revocation in the selected library; count offline members too |
| Device description | Hardware, label and connection fields share a library row | Device publishes its hardware facts; library label overrides are separate shared preferences; current connectivity is observed locally |
| Source registration | `sources` table has no `Syncable` registration and no explicit ingest-authority field | Shared library descriptor with stable source UUID, origin, capture policy and explicit ingest authority; distinguish subscriber metadata from a local writer |
| Volumes | Old device-owned sync model; one row mixes identity, device, mount path and statistics | Stable volume identity plus device-scoped attachment observations; remote paths remain qualified by their observing device |
| Locations/pins | Source-relative rows; registered under old device-owned sync with a missing source dependency | Remove without migration or compatibility support; new navigation uses Space items and processing intent uses policy-owned source-relative targets |
| Spaces, groups and items | Old shared-row sync; items still contain `entry_uuid` | Keep current shared organization, migrate targets to record/source addressing, and preserve explicit local presentation preferences |
| Tags and annotations | Legacy tag tables plus source-store overlays; FD2 migration pending | Definitions/applications live with sources as specified by FD2; assertion changes merge independently from generation transfer |
| Library settings and policies | Settings mainly persisted in local config; sync flag not wired to lifecycle | Explicit shared-settings allowlist; local watcher permissions, performance settings and secrets retain local ownership |
| Jobs and operation history | Library-owned durable jobs and remote operation/log access | Keep execution state at the executing device; optionally exchange summaries, never execute a replicated job row automatically |
| Credentials and caches | Local encrypted credentials and machine artifact layout | Replicate permitted descriptors and cache intent; keys, credentials, cache paths and open handles remain local |

The relevant entity implementations are in
[`device.rs`](../../core/src/infra/db/entities/device.rs),
[`source.rs`](../../core/src/infra/db/entities/source.rs),
[`volume.rs`](../../core/src/infra/db/entities/volume.rs),
[`location.rs`](../../core/src/infra/db/entities/location.rs), and
[`space_item.rs`](../../core/src/infra/db/entities/space_item.rs).

Pins need particular care. Their current `Syncable` implementation inherits
default empty backfill and successful no-op apply/delete methods from
[`syncable.rs`](../../core/src/infra/sync/syncable.rs). Its dependency names
`source`, which has no registration. The current
[`location::pin` and `unpin`](../../core/src/location/mod.rs) write rows without
emitting sync changes. Their declaration alone does not make them work across
devices.

[Locations Research](2026-09-15-locations-research.md) audits duplicate Space
shortcuts, local-only resolution, setup UI and the missing replacement for
`location.job_policies`. The agreed addressing boundary accepts `SdPath` at
policy creation, persists a separate source-relative target, and resolves an
execution address through the source layer. `SdPath` gains no source variant.
Space items own navigation and may reuse that target value. This is a breaking
release: remove locations without migrating rows, preserving IDs or supporting
old operations. Shared processing intent does not imply executing replicated
jobs on every device.

Source assertions remain durable in source stores. Library sync can establish
permission and exchange assertion progress without moving their authoritative
storage back into the library database. See
[Source Durability](../core/design/source-durability.md) and
[Entries Final Drop](2026-09-15-entries-final-drop.md).

## Existing modularity and what to reuse

The existing sync system already uses the same registration pattern as ops.
[`Syncable`](../../core/src/infra/sync/syncable.rs) lets each database model
define its identity, serialized fields, dependencies, foreign-key mapping,
catch-up query and incoming apply/delete behaviour.
[`register_syncable_shared!` and `register_syncable_device_owned!`](../../core/src/infra/sync/registry.rs)
submit those implementations through `inventory`. The shared engine dispatches
through registered functions. Domains do not each need a transport or retry
loop, which is the right separation to preserve.

For example, `space` implements shared-change application and catch-up;
`volume` implements device-owned state application, catch-up and deletion.
Both register themselves in their entity modules. The new source registry
can join this framework once its ownership and payload are defined.

The entry-era coupling is concrete, but it does not describe every sync file:

| Area | Existing coupling | Proposed treatment |
|---|---|---|
| Library write helpers | Special handling adds directory paths to entry payloads | Remove the entry branches after their callers retire |
| Backfill | Explicit `model_type == "entry"` branches order roots and resolve self-referencing parent FKs individually | Remove the entry branches; retain bounded catch-up for surviving models |
| Rebuild hooks | Entry closure and directory projections rebuild after large imports | Retire obsolete model hooks; retain the extension point if a surviving model needs it |
| Model registrations | Entries, old content/media and entry-bound junctions still participate | Remove through the final-drop register after assertion migration |
| Dependency handling | Generic UUID dependencies and integer/UUID FK conversion support entry trees and other relationships | Keep what device/volume/source, pins and organization still require; do not remove solely because comments mention entries |
| Transport and progress | Library multiplexer, HLC, peer logs, acknowledgements, retries, batching and metrics | Reuse and test against the reduced model set and corrected membership contracts |

At this inspection, `infra/sync` contains 7,256 lines and `service/sync`
contains 8,315, including comments and tests. `peer.rs` is 3,038 lines and
`backfill.rs` is 1,496. These measurements show the review surface; they are
not estimates of removable code. A log entry named `entry` is not necessarily
a filesystem entry.

The recommended implementation is an evolution of this framework. The
improvements are enforceable library membership, an explicit surviving model
set, reliable join completion, atomic application-write/delivery intent and
complete domain implementations. The current trait's successful no-op defaults
should become required implementations for the capability a model registers,
or explicit unsupported capabilities. Registering a model must not silently
promise backfill and apply behaviour it does not implement.

Extend the existing write/transaction API to support the durability contract
before introducing parallel delivery machinery. Preserve domains' ownership
of serialization and application rules. Shared retry, ordering and transport
remain shared infrastructure. The research has not established a reason for
a wholesale sync-engine replacement, nor measured a reliability improvement;
the failure and convergence tests below must establish that improvement.

## Proposed model

Pairing establishes who a peer is. Joining establishes which library the peer
belongs to and what it may do. The interface can present both as one operation,
such as adding a device to the current library, while preserving both durable
results and showing partial completion accurately.

For the user's Mac and NAS, the desired result is one chosen library UUID on
both devices, with both devices registered in both copies of that library.
Its source list describes all registered sources even if none of their arenas
or replicas are loaded. Metadata availability and byte availability are then
reported independently for each source.

Keep the implementation small:

1. Use the existing library database as the membership and registration
   authority. Evolve its device representation rather than adding another
   competing device catalog. Separate device-published facts, membership
   authorization and local connection state in the read/write contracts.
2. Give source descriptors an explicit ingest authority. A remote source row
   must not automatically start an adapter, scan or watcher. Keep source
   subscription intent and received-generation state separate from registration.
3. Identify source requests by library and source, and resolve paths at the
   serving device. Scope listings, summaries, snapshots, queries, bytes and
   operations to the same membership/grant. Keep machine-level caches reusable
   while checking library access on every route to them.
4. Replicate a small allowlist of library records using typed payloads, stable
   UUIDs, revisions and durable removals. Reuse the existing transport, library
   routing and HLC primitives where they satisfy the new contracts. Retire
   entry hierarchy backfill and its dependency machinery.
5. Record each local shared change and its delivery intent in the same SQLite
   transaction. A small outbox beside the library rows is sufficient; source
   assertions need equivalent atomicity in their own store. A process crash
   between writing a row and broadcasting must not lose the change.
6. Exchange progress per writer/stream, replay idempotently and acknowledge
   committed batches. HLC orders applicable shared edits; it is not evidence
   that every change from every writer was received. Missing source generations
   cannot block small library metadata from converging.

The atomicity rule addresses an existing gap. For example,
[`spaces.update`](../../core/src/ops/spaces/update/action.rs) commits its row
before calling `sync_model`. The current
[`TransactionManager`](../../core/src/infra/sync/transaction.rs) appends shared
changes to a separate peer log, while device-owned commits emit an event.
Those paths do not make the earlier application write and delivery record one
transaction. Shrinking the model set alone does not fix this failure window.

Generation updates keep one ingest authority and their own revisions.
Ordinary shared preferences can use the established logical-time merge rules.
Membership grants and revocation need authorized issuers and explicit conflict
rules so an ordinary device update cannot restore revoked access. Live
connectivity remains an observation, with a timestamp and expiry, rather than
a permanently synced `is_online: true` flag.

## User-visible agreement

The primary flow should ask which library the new device is joining as part of
adding it. Successful completion means both sides have persisted membership
for that same UUID. Interrupted registration remains visible and resumable.

Three intentions remain distinct:

| Intention | Effect |
|---|---|
| Add my device to this library | Both devices become members of one library; its shared descriptors and permitted state converge |
| Receive a source or folder from another library | An explicit grant makes that content available; it does not make the recipient a member of the entire foreign library |
| Operate a remote library | A scoped remote session targets that library; the caller's local library is not silently merged |

These match the useful distinctions in
[Shares and Remote Libraries](../core/design/shares-and-remote-libraries.md).
Its older transport recommendations are historical; current Iroh transport
already works and should be reused.

The library overview should count its non-revoked members, including offline
ones, and separately show how many are connected. Its source and volume totals
must use the same library scope. A paired device outside the library belongs
in network/device setup, or an explicit external-share view, with its status
explained.

A joined library with the NAS offline should still show two devices and the
NAS's known sources. It can show the NAS offline and describe which saved
metadata is usable. A failed snapshot download changes availability, not
membership or source existence.

## UI setup flow audit

The setup flow still exists as
[`SyncSetupModal.tsx`](../../packages/interface/src/components/modals/SyncSetupModal.tsx),
but it has no callers in the current application. It selects a paired device,
discovers that device's libraries, offers share or join, confirms the choice
and calls `network.sync_setup`. The frontend and backend operation definitions
remain available.

Commit `2c7e058ce`, September 15, 2026, removed the `useSyncSetupDialog` import,
handler and Setup Sync button from `routes/overview/OverviewTopBar.tsx` during
overview simplification. The dialog file survived. Searching the current
application finds its exported opener only in its own definition.

The surviving [library switcher](../../packages/interface/src/components/SpacesSidebar/LibrarySwitcher.tsx)
offers Pair a Device, wired through the sidebar to
[`PairingModal`](../../packages/interface/src/components/modals/PairingModal.tsx).
That modal shows pairing success and closes after two seconds. It does not
continue to library selection or membership registration.
[`LibrarySettings`](../../packages/interface/src/Settings/pages/LibrarySettings.tsx)
offers a Sync Enabled toggle, with no setup or membership controls. The
`SyncMonitorPopover` implementation also survives without a mounting caller;
its presence in the tree is not an available user flow.

Restore setup as part of the library-sync work. Reuse the surviving dialog's
useful steps and generated operation types, then update it for the corrected
membership contract:

1. Provide an Add Device / Set Up Sync entry point from the current library
   menu and library settings. Allow selecting an already paired device, which
   covers the Mac/titan state without pairing again.
2. Continue a newly completed pairing into library setup with that peer
   selected. Keep the chosen library visible and offer a deliberate exit for
   network-only pairing.
3. Show whether the peer already belongs to this library, can join it, or has
   a different library. Distinguish equal names with device and library
   identity. Explain preservation/adoption of existing data before any merge.
4. Render durable membership progress and retry partial joins. The existing
   modal closes on mutation success without inspecting the membership result;
   completion must use the corrected backend's verified outcome. Expose a
   retry path after errors instead of leaving only a failed execution screen.
5. Refresh library membership, sources, volumes and overview state after
   completion. When joining another library, select the resulting library
   explicitly so the user is not left viewing their previous one.
6. Keep membership and sync status reachable after setup. A member can be
   offline or catching up without vanishing, and a paired non-member needs a
   visible way to finish setup.

The old dialog also contains an `as any` cast, debug logging and a deprecated
leader field. Reusing its source does not establish compatibility or correctness;
update it against generated types and the repaired operation contract. This
audit checked source references and git history, not a browser interaction.
It did not restore the button or execute setup against either live device.

## Migration and implementation boundaries

The current two libraries already contain data. Sharing either library to the
other device creates another library with the shared UUID; it does not merge
the two existing registrations and assertions. The existing merge action is
unimplemented, and the global cache must be made library-aware before relying
on multiple open libraries.

Choose a canonical library for the desired shared environment, then prepare
an explicit import/adoption of the other library's sources and shared objects.
Preserve source, record and volume identities, assertions, pins and NAS
freezes. Handle object conflicts and library-name/path collisions in a
reviewable migration report. Do not rewrite library IDs in place, recreate
source stores or rescan origins to make the libraries appear joined.

A useful sequence for a subsequent execution plan is:

1. Define membership, owner/observer fields, source grants and shared versus
   local data. Add tests for two libraries on one daemon before wiring new sync.
2. Repair the join/share handshake and target-library resolution. Use durable
   pending state and retries, and verify registration on both sides before
   reporting completion. Restore the setup entry points and pairing continuation
   described above. Make the sync setting correspond to actual behaviour.
3. Implement small library-state replication for members, volumes and source
   descriptors. Route source transfer and summaries through that membership.
4. Add shared pins and surviving organization, then source assertions as FD2
   permits. Verify atomic local changes, offline replay and removals.
5. Migrate the Mac and titan through a separately reviewed operation, then
   verify the overview, device/volume lists, offline restart and source access.

The [source runtime plan](2026-09-15-source-runtime-reliability.md) still owns
durable-write correctness, arena recovery and generation delivery. Its R3
inventory, R5 counts and R6 peer format need this library scope. The agreed
arena-first, SQLite-fallback read strategy remains unchanged.

The [entries teardown](2026-08-20-entries-teardown-execution.md) and final-drop
plan remain the deletion registers. Removing old row sync must preserve or
replace the small library contracts identified here. Do not restore the old
entry registrations to make the new library agreement work.

## Acceptance cases to carry into the plan

- Pairing without joining does not grant all libraries' sources.
- Joining yields the same library UUID and both member rows on both devices.
- Setup is reachable from the current library and from an already paired
  non-member; completing pairing can continue directly into that flow.
- Equal library names do not conceal different identities or imply a merge.
- Failed/partial setup remains retryable; verified completion selects the
  intended library and refreshes membership and overview state.
- Lost requests, lost acknowledgements, crashes and repeated joins converge
  without duplicate members or false completion.
- A member missing a source snapshot still sees its descriptor and owner.
- An offline member remains counted; connection and source availability can
  change independently.
- Two libraries on one daemon do not exchange source lists, summaries, grants
  or mutations through process-global cache state.
- A NAS mount path never becomes a local Mac watcher root after metadata sync.
- Moving a volume preserves identity while changing attachment and, through an
  explicit handoff, ingest authority.
- A create/update/delete made offline survives restart and converges; stale
  replay cannot resurrect a deletion or revocation.
- A shared pin resolves by source and relative path on both devices without
  requiring an old entry row or the origin to be mounted locally.
- A record or job visible remotely is not automatically re-ingested or
  executed on the receiving device.
- Replicated source descriptors and cached replicas do not multiply overview
  file counts or capacity totals.
- Old/new protocol combinations reject unsupported scopes explicitly and
  preserve the existing working remote operations through reviewed migration.

## Decisions for discussion

The main user-flow decision is whether adding an owned device defaults to
joining the current library. Recommended: yes, with the selected library
visible in the agreement. Basic network pairing remains available for scoped
sharing or remote operation without membership.

The other decisions are the shared/local policy for pins and presentation,
who may admit or revoke members, how the sync pause setting affects metadata
replication versus remote serving, and how to adopt the two existing libraries
without losing either one's state. None requires putting file entries back
into the library sync stream.
