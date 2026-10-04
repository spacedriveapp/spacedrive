# Addressing: Stable Names and One Resolver

> Status: proposed 2026-09-25. P0 and P1 are ready to start; D1 to D6 are
> James's.
> Captured: 2026-09-25, measured against `66b4a0c14`
> Owns: what an address names and how it resolves: device and cloud identity
> in `SdPath`, the content reference, the durable source-relative target and
> record reference, the resolver with its scope and version checks, the text
> form, and the content-row rules that content addresses depend on
> Register: `PROJECT_STATUS.md`
> Companions: `docs/core/addressing.mdx` (rewritten in P0),
> `2026-09-15-locations-research.md` (option 1: the durable target stays out
> of `SdPath`), `2026-09-16-add-to-library.md`,
> `2026-09-17-tags-on-source-stores.md`, `2026-09-22-action-previews.md`,
> `2026-09-23-file-operations.md`, `2026-09-21-opening-remote-files.md`,
> `2026-09-08-locations-demoted.md` (L5 sidecars),
> `docs/core/design/mounts.md`, `docs/core/cloud-integration.mdx`

## Outcome

An address names one place, and resolving it later reaches the same place or
fails with a reason.

- `SdPath` is a locator. It names a device or a cloud volume by UUID, plus a
  path or key in that origin's own syntax.
- Content has its own reference type, accepted only by reads that can use any
  copy. Derived artifacts get a reference type when the next sidecar kind
  needs one.
- Anything persisted names a source and a relative path, or a source and a
  record. Device paths are resolved from those when work runs.
- One resolver in the source layer turns every address into something an
  operation can act on. It checks availability, the caller's scope and the
  expected version, and fails as not found, offline, denied, stale or
  ambiguous.
- A content row's integrity hash was computed from the bytes of every record
  that points at it, so no deletion rests on another file's hash.
- `docs/core/addressing.mdx` describes the code after every phase.

## Where it stands

### What holds

- Record UUIDs are v7, assigned by the store (`crates/store/src/file.rs:316`),
  and survive rescans and watched renames. Store rows are source-relative, and
  sources anchor to a volume UUID and a relative root
  (`core/src/ops/indexing/sources.rs:10-25`).
- Content UUIDs are `v5(CONTENT_NAMESPACE, hash)`
  (`crates/store/src/content.rs:24-31`), so separate installs agree without
  coordination.
- Compare-delete and dedupe compare integrity hashes before removing a copy
  (`core/src/ops/files/delete/compared.rs:8-15`).
- Option 1 keeps `SdPath` independent of source registration, so an untracked
  path still has an address.

### `SdPath` names places with values that change

- **Device slug.** Generated from the device name
  (`core/src/domain/device.rs:221`) and unique only within one `library.db`.
  A collision rename is a per-library override
  (`core/src/device/config.rs:26`) that `SdPath::local` never reads; it writes
  the process-wide slug (`core/src/domain/addressing.rs:194-200`).
  `devices.list` returns the library row's slug and `core.status` the global
  one.
- **Three spellings of this device.** `is_current_device` accepts `"local"`,
  the slug and the UUID string (`addressing.rs:204-208`). The derived `Eq` and
  `Hash` treat them as different values. The CLI sends `"local"` for every file
  command (`apps/cli/src/domains/file/args.rs:318-328`), and so do the Add
  Storage modal
  (`packages/interface/src/routes/explorer/components/AddStorageModal.tsx:508,552`)
  and the analyzer (`packages/interface/src/routes/analyzer/index.tsx:203`).
  These places compare against the global slug with `!=`:
  - directory listing and search return empty results
    (`core/src/ops/files/query/directory_listing.rs:180`,
    `core/src/ops/search/arena_search.rs:61`);
  - thumbstrips refuse the path (`core/src/ops/thumbs/thumbstrip.rs:36`);
  - copy routing treats two spellings of this device as two devices
    (`core/src/ops/files/copy/routing.rs:55,144,336,434`);
  - a move keeps its source when the spellings differ
    (`core/src/ops/files/copy/job.rs:684`).
- **Slug edits.** `device.update` rewrites `device.json` only
  (`core/src/ops/devices/update.rs`), and the slug is read once at startup
  (`core/src/lib.rs:128-129`). After an edit and a restart, every persisted
  `SdPath` with the old slug is treated as another device's.
- **Cloud.** `(service, identifier)` carries no account or endpoint. The
  fingerprint hashes the service name and the bucket, container or root
  (`core/src/ops/volumes/add_cloud/action.rs:410-412`). Drive, OneDrive and
  Dropbox default the root to `"root"`. Two accounts, or one bucket name on AWS
  and on R2, share a fingerprint. The second add overwrites the first's
  credential (`core/src/crypto/cloud_credentials.rs:80-97`) and replaces its
  volume in the shared map (`core/src/volume/manager.rs:1403-1421`). Cloud
  volume ids are random per add (`add_cloud/action.rs:418`). Resolution is a
  string lookup on the volume manager that every library shares
  (`manager.rs:1339-1373`), and its fallback calls `blocking_write()` on a
  tokio lock inside async code (`manager.rs:1365`), which panics.
- **Path syntax.** `PathBuf` applies the reading machine's rules to another
  device's path. On a Mac, a Windows device's `C:\Users\x\f.txt` has no parent
  and `join` inserts `/`. `from_uri` produces `/C:\Users\...`
  (`addressing.rs:446-455`). Names that are not valid UTF-8 cannot serialize.

### One enum holds different kinds of value

`Physical` and `Cloud` are locators. `Content` matches any copy of some bytes.
`Sidecar` names a derived file in a tree nothing writes. Every operation
accepts all four, and they disagree about what the other variants mean:

- `files.copy` with `move_files` skips validation for non-local sources
  (`core/src/ops/files/copy/preflight.rs:134-154`) and moves whatever
  `resolve_in_job` picks for a `Content` source, the first local copy that
  exists (`addressing.rs:693-709`). `files.createFolder { items }` dispatches
  the same move with no validator
  (`core/src/ops/files/create_folder/action.rs:110-127`).
- A `Content` or `Sidecar` copy destination panics the job:
  `destination.join()` runs (`copy/job.rs:1289`) before the destination is
  resolved (`:349`).
- Deleting a `Cloud` path permanently passes validation and deletes nothing.
  The router sends any batch with a non-local path to `RemoteDeleteStrategy`
  (`core/src/ops/files/delete/routing.rs:10-23`), which groups paths by the
  deprecated `device_id()`, always `None`
  (`core/src/ops/files/delete/strategy.rs:378-383`). A batch mixing local
  files and a cloud path deletes none of them.
- A move to another device copies and leaves the source, because source
  deletion requires equal slugs (`copy/job.rs:684`), while preflight reports a
  move.
- Nothing outside tests constructs `Content` or `Sidecar`. The frontend only
  formats `Content` (`packages/interface/src/routes/explorer/utils.ts:64`).

### Nothing durable has a type

Option 1 persists `{ source_id, relative_path }` and resolves it when work
runs. `2026-09-15-locations-research.md:263-305` leaves the type unnamed, and
nothing implements it. What remembers a file today:

- Space items store `SdPath` JSON with the slug
  (`core/src/ops/spaces/add_item/action.rs:92-103`) and sync it verbatim
  (`core/src/infra/db/entities/space_item.rs:50-68`). One item that fails to
  parse fails the whole layout (`core/src/domain/space.rs:358,496`).
- Tags take bare record UUIDs. The daemon finds the owning store by scanning
  every arena partition, then every store, then replicas
  (`core/src/ops/tags/targets.rs:218-299`).
- Adapter records have no address. The UI renders them with no actions
  (`packages/interface/src/components/Sources/SourceDataRow.tsx:26-57`), and
  tag resolution only loads filesystem sources
  (`core/src/ops/indexing/volume_index.rs:263`).
- Inputs typed `PathBuf` drop the device. "Add to Library" sends a remote
  folder's path to `sources.track`
  (`packages/interface/src/routes/explorer/hooks/useFileContextMenu.ts:289-297`),
  which tracks that path on this machine if it exists.

### Resolution is scattered, and network admission is too broad

- `PathResolver` (`core/src/ops/addressing.rs`) has no callers.
  `resolve_in_job` is the only resolver in use. It handles `Content` only,
  looks at local copies only, ignores the tier, and calls a blocking `exists()`.
  Each operation checks `as_local_path()` itself, behind seven separate
  `*.remote_root` codes.
- remote_ops admits any node in the registry's node map
  (`core/src/service/network/protocol/remote_ops.rs:302-315`). A device enters
  that map when pairing starts
  (`core/src/service/network/device/registry.rs:301`). The call then runs as
  this daemon's own session (`core/src/infra/daemon/rpc.rs:545-550`), whose
  permissions are `admin_all()` (`core/src/infra/api/permissions.rs:284-286`).
  Byterange admits the same set
  (`core/src/service/network/protocol/byterange.rs:921-933`).
- file_transfer admits any node. An unknown one gets a random device id
  (`core/src/service/network/protocol/file_transfer.rs:1520-1537`), and the
  event loop routes the protocol without a pairing check
  (`core/src/service/network/core/event_loop.rs:373,469`). Pull serves any
  existing file under a registered source root and ignores the requester
  (`file_transfer.rs:1156-1177`).
- Path checks and opens use different paths. Push validates one spelling and
  reopens the raw string with `create(true)` on every chunk
  (`file_transfer.rs:914-969`). Pull streams the path it did not canonicalize
  (`:1245,1301,1318`). Byterange's `authorize_path` is lexical, and the open
  follows symlinks (`byterange.rs:404-422,529-568`).
- No address carries an expected version. `journal::Subject` (size, mtime,
  is_dir) is captured after each mutation and checked only by undo
  (`core/src/ops/files/undo/reverse.rs`). `files.trash_empty` purges recorded
  locations without checking it (`core/src/ops/files/trash_view/empty.rs:57-87`).

### A record can carry an integrity hash its bytes don't have

`content.sampled_hash` is `UNIQUE` (`crates/store/src/record.rs:79`), so every
record with one sampled hash shares one row. `bind_content` upserts on it
(`crates/store/src/db.rs:1165-1174`). An incoming integrity hash overwrites
the row's, and a sampled-only write keeps the row's. Three paths follow:

1. **A file changed in place.** A size or mtime change clears the record's
   content (`crates/store/src/file.rs:666-674`), but rehashing binds it back to
   the same row by sampled hash, and that row keeps the old integrity hash. A
   change outside the roughly 56 KB the sampled hash reads leaves the sampled
   hash equal. Fixed-size VM disks and encrypted containers change this way.
2. **Two files with one sampled hash.** Both point at one row, and each read
   overwrites the other's hash.
3. **Orphaned rows.** Nothing deletes content rows, so a new file with an old
   file's sampled hash inherits the old file's integrity hash.

`integrity()` trusts a stored hash without reading the file
(`core/src/ops/files/delete/compared.rs:277-280`), and dedupe compares two such
values (`core/src/ops/files/delete/duplicates.rs:350-351`). Dedupe writes
what a batch read back to the store (`duplicates.rs:321`) and pages 1000 files
at a time (`core/src/ops/paths/compare.rs:52`), so a single run can read back a
hash it wrote for a different file. A VM disk that was read in full once, then
changed, then compared against a backup taken before the change is removed as a
copy of that backup. This breaks `docs/core/data-model.mdx:189`.

The row's uuid also changes when the integrity hash lands (`db.rs:1169`).
Content-keyed tag assertions are bound once
(`crates/store/src/tags.rs:182-195`) and never rebound, so a content tag
applied before verification stops reaching the other copies afterwards. No
test covers a changed file, a collision, an orphan, or a tag across the
upgrade.

The store has no schema version. `RECORD_SCHEMA` is `IF NOT EXISTS` statements
(`record.rs:25-26`), the only `ALTER TABLE` adds facet columns
(`db.rs:140-162`), and replicas are `VACUUM INTO` copies of the owner's store
(`core/src/service/network/protocol/byterange.rs:747-750`).

### Serialization and text form

- `SdPath`'s hand-written `Deserialize` (`addressing.rs:62-145`) disagrees
  with its derived `Serialize`:
  - job state is rmp with binary UUIDs, and the deserializer reads
    `content_id` as a string, so a copy job with a `Content` source cannot
    resume;
  - `CloudServiceType::Other` serializes as `"cloud"`, which `from_scheme`
    rejects (`core/src/volume/backend/mod.rs:88-89,113-126`);
  - `SidecarFormat::MessagePack` serializes as `"message_pack"`, which its
    `TryFrom` rejects (`core/src/domain/sidecar.rs:155-168`). The generated
    TS type says `"mp_4"` where serde writes `"mp4"`.
- `from_uri` hard-codes five sidecar directories (`addressing.rs:477-484`) and
  cannot parse two that `display()` writes.
- Rust `display()` writes `local://slug//Users/...`, and TS `sdPathToUri`
  writes `local://slug/Users/...` (`utils.ts:52-72`). Neither percent-encodes.
  The PathBar's URI mode fills in a URI and parses the result with
  `JSON.parse` (`PathBar.tsx:786-789`).
- Cloud indexing queues subdirectories as full URIs and then prefixes them
  again, producing `s3://bucket/s3://bucket/dir/...`
  (`core/src/ops/indexing/phases/discovery.rs:418-423,578-583,686-696`).

### Where the doc and the code differ

`docs/core/addressing.mdx` makes four claims the code doesn't back:

- It says credentials come from the authorized source. No source is consulted
  for cloud paths.
- It says the content resolver weighs confirmation, availability, permissions
  and policy. It takes the first local copy.
- It says clients can tell denied, stale and ambiguous apart.
  `PathResolutionError` has none of the three.
- It says a drive can remount elsewhere without breaking anything. Nothing
  re-anchors a source while the daemon runs; `SourceRegistry::remount` has no
  caller (`core/src/ops/indexing/sources.rs:370-376`).

## Decisions

D1 to D6 are James's. Each states the recommendation first.

**D1. `Physical` names its device by UUID.** The slug stays as display text
and as CLI input, resolved when a request arrives. `"local"` leaves the wire.
The alternative is an immutable slug that is unique across every device, which
no single device can enforce.

**D2. `SdPath` holds locators only.** `Content` moves to its own `ContentRef`
type, accepted by reads that can use any copy (P5). `Sidecar` goes with it
(D3). The alternative keeps the four variants and adds a locator-only type for
destinations and destructive inputs. That leaves every other operation to
decide what `Content` means.

**D3. No sidecar address until the next sidecar kind needs one.** Remove
`SdPath::Sidecar` and everything that serves the content-keyed tree:

- the `/sidecar/...` routes (`apps/server/src/main.rs:157-221,571`,
  `apps/tauri/src-tauri/src/server.rs:69-152,270`);
- the `get_sidecar_path` command (`apps/tauri/src-tauri/src/files.rs:53-85`);
- the frontend branches gated on `File.sidecars`, which no producer fills
  (`core/src/domain/file.rs:255,311,403`).

Thumbnails keep their record-keyed tier. L5 defines `SidecarRef`, keyed by
record or confirmed content as `docs/core/virtual-sidecars.mdx:48-55`
specifies. The alternative keeps the routes for L5, serving a tree nothing
writes until then.

**D4. Pairing is full trust, and the doc says so.** A remote call runs as the
calling device's session with admin permissions, and only fully paired
devices are admitted. The doc drops "granted scope". Scoped grants arrive with
sharing to other people. The alternative limits paired devices to registered
sources on the executing device, the rule file_transfer applies today. That
also blocks tracking a new source on a NAS from the Mac, unless source
administration is exempt.

**D5. Cloud objects are addressed by a cloud volume UUID.** The UUID derives
from the service, the endpoint or account, and the container (P6). This holds
whether cloud storage stays a volume or becomes a source adapter as
`docs/core/cloud-integration.mdx:7-9` describes, because a cloud source would
anchor to that volume the way filesystem sources anchor to drives.

Google Drive stays path-addressed through OpenDAL, which takes the first match
when a name repeats in a folder (`opendal-0.54.1/src/services/gdrive/core.rs:428`).
Same-named files in one folder stay unreachable until a Drive ingest keys
items by file id. Document the limit.

**D6. A move across devices is refused at preflight** with
`move.cross_device`, until the transfer strategy deletes the source after the
receiver confirms the checksum. The alternative implements that deletion now,
inside the transfer strategy.

## Target model

```rust
/// Where bytes are: a device and a path in that device's syntax, or a cloud
/// volume and an object key.
pub enum SdPath {
	Physical { device_id: Uuid, path: String },
	Cloud { volume_id: Uuid, key: String },
}

/// Any copy of some bytes. Accepted by reads only.
pub struct ContentRef {
	pub content_id: Uuid,
}

/// A path inside a registered source, relative to its root.
pub struct SourcePath {
	pub source_id: Uuid,
	pub path: String,
}

/// One record in one source's store.
pub struct RecordRef {
	pub source_id: Uuid,
	pub record_id: Uuid,
}
```

- `Physical.path` is absolute, in its owner's syntax. The syntax comes from the
  path's prefix: `/` for Unix, a drive letter or `\\` for Windows. `parent`,
  `file_name` and `join` apply that syntax on any host, and `as_local_path()`
  returns a `&Path` only on the owning device. The `typed-path` crate
  implements this if a dependency is preferred over a small module.
- `SourcePath.path` is `/`-separated with no root, parent component or
  platform prefix, the containment rule in `PROJECT_STATUS.md`'s safety
  invariants. It follows a remount. It does not follow a directory renamed
  inside the source (`locations-research.md:300-304`).
- `SdPath` derives `Deserialize`. The hand-written implementation goes.

### Resolution

One resolver in the source layer, the one `locations-research.md` describes.
It converts:

- `SdPath` to `SourcePath`: the innermost registered source that holds the
  path on its device, through the local registry or a replica's reach;
- `SourcePath` to `SdPath`: the source's device and current root, plus the
  relative path. A target that can't be resolved is kept with its reason,
  never dropped;
- `SdPath` to `RecordRef` and back, through the named store. An adapter record
  resolves to its record, with no `SdPath`;
- `ContentRef` to one holder (P5).

Every operation resolves its inputs here, passing the caller's session and what
it intends to do. Failures become findings with one shared set of codes, which
replaces the seven `*.remote_root` constants:

| Code | Meaning |
|---|---|
| `address.not_found` | Nothing is at the address |
| `address.offline` | The device, volume or source is unreachable now |
| `address.denied` | The caller's session may not act on it |
| `address.stale` | The file changed since the caller last saw it |
| `address.ambiguous` | A content reference matches different bytes |
| `address.other_device` | The operation runs on the owning device; use `--device` |
| `address.unsupported` | The operation cannot act on this kind of address |

### Text form

One codec in Rust. A fixture of cases is read by both the Rust and the TS
tests.

| Value | Form |
|---|---|
| `Physical` | `device://<device-uuid>/<path>` |
| `Cloud` | `cloud://<volume-uuid>/<key>` |
| `ContentRef` | `content://<content-uuid>` |
| `SourcePath` | `source://<source-uuid>/<path>` |
| `RecordRef` | `record://<source-uuid>/<record-uuid>` |

Path components are percent-encoded as RFC 3986 `pchar` and joined with `/`. A
Windows path's prefix is its first component (`device://<id>/C:/Users/x`). The
parser rejects a string with no scheme; the CLI turns plain paths into
addresses itself.

## Phases

Each phase updates `docs/core/addressing.mdx` for what it lands, in the same
commit. Each regenerates the TypeScript and Swift clients after changing public
types.

### P0. Make the doc describe the code

Rewrite `docs/core/addressing.mdx` to match what HEAD does:

- the four variants and the three spellings of this device;
- `resolve_in_job` as the only resolver;
- pairing as the trust boundary;
- content ids that change when the integrity hash lands.

Mark the target model as planned and link this plan.

Acceptance: every statement in the doc holds at HEAD.

### P1. Stop the data loss and fix network admission

Three independent tracks. P1c's cross-device refusal waits on D6.

**P1a. Content rows record each file's own bytes.**

- **Schema version.** Source stores get a schema version in
  `PRAGMA user_version`. The writer runs ordered migrations at open
  (`crates/store/src/source.rs:158-176`), before `revision::install`.
  - A read-only open of a local store below the current version opens the
    writer first.
  - Replicas keep the owner's version, since `VACUUM INTO` copies it. Readers
    handle both shapes until the owner upgrades; a test confirms the version
    survives the copy.
- **Migration 1** rebuilds `content`, keeping each row's `id` so every
  `record.content_id` still holds:

  ```sql
  CREATE TABLE content (
      id INTEGER PRIMARY KEY,
      uuid BLOB NOT NULL,
      candidate_uuid BLOB,
      sampled_hash TEXT,
      integrity_hash TEXT UNIQUE,
      size INTEGER,
      kind INTEGER
  );
  CREATE UNIQUE INDEX idx_content_candidate ON content(sampled_hash)
      WHERE integrity_hash IS NULL;
  CREATE INDEX idx_content_sampled ON content(sampled_hash);
  CREATE INDEX idx_content_uuid ON content(uuid);
  CREATE INDEX idx_content_candidate_uuid ON content(candidate_uuid);
  ```

  The migration also:
  - demotes every row that has both hashes, since the store can't tell which
    of them are wrong: it clears the integrity hash and returns `uuid` to
    `uuid_for(sampled_hash)`;
  - sets `candidate_uuid` to `uuid_for(sampled_hash)` on every row that has a
    sampled hash;
  - deletes rows that no record references;
  - merges rows that would collide on the new unique index;
  - gives assertions anchored on a record whose row was demoted the candidate
    uuid as their content key, in both `tag_assertion` and `record_overlay`.
    This is the only change this plan makes to assertion rows.

  The rebuild runs with foreign keys off (`source.rs:30`) and reinstalls the
  revision triggers afterwards.
- **`bind_content`** (`db.rs:1151-1205`):
  - with an integrity hash, it upserts on `integrity_hash` and binds the record
    to that confirmed row;
  - with a sampled hash only, it upserts on the candidate index and never
    touches a confirmed row.

  When a record moves from its candidate row to a confirmed row with the same
  sampled hash, the same transaction rewrites the content key of assertions
  anchored on that record.
- **Moves.** A `Moved` resolution keeps the content only when size and mtime
  both match. Today either one is enough (`file.rs:307-313`).
- **Readers.**
  - `PENDING_VERIFICATION` (`file.rs:788-795`) claims records on candidate
    rows whose sampled hash is held by more than one record, confirmed rows
    included. The second copy is then still read after the first one
    confirms.
  - Duplicate discovery groups by sampled hash (`crates/store/src/read.rs:448-468`,
    `file.rs:895-943`) and splits a group by integrity hash where its members
    carry one.
  - Lookups by content uuid match `uuid` or `candidate_uuid`
    (`file.rs:965-1001`, `tags.rs:302-448`).
- **Tests.**
  - A file changed in place with its sampled hash unchanged loses its
    integrity hash.
  - Two files with one sampled hash and different bytes end on two confirmed
    rows once both are read.
  - Dedupe over two such files keeps both, across two runs and across pages.
  - Compare-delete keeps a file that changed after its copy was verified.
  - A new file doesn't inherit an orphaned row's integrity hash.
  - A content tag applied before verification reaches every copy afterwards.
  - The migration demotes rows, keeps every `record.content_id`, rewrites
    anchored content keys, and leaves assertion rows otherwise untouched.
  - Reopening a migrated store runs nothing and rewrites no triggers.

  `the_integrity_tier_renames_the_content_without_moving_the_row`
  (`crates/store/tests/record.rs:598`) changes along with the rule it asserts.

Acceptance: a stored integrity hash on a record's row was computed from that
record's bytes since its last change, and `data-model.mdx:189` holds.

**P1b. Admit paired devices only, and open the path that was checked.**

- **One admission check.** A registry method returns the device for a node
  only in the `Paired`, `Connected` or `Disconnected` states, the set
  `event_loop.rs:228-236,326-336` already treats as paired. remote_ops,
  byterange and file_transfer all use it. file_transfer refuses an unknown
  node instead of minting an id for it.
- **remote_ops.** The call runs as the calling device's session.
  `execute_json_operation_with_context` takes the session as an argument
  instead of building this daemon's (`rpc.rs:534-550`). Permissions follow D4.
- **file_transfer.** It canonicalizes once and keeps the canonical path in the
  session. It opens that path once per transfer, without following a symlink
  at the leaf, and pull streams the path it checked.
- **byterange.** It canonicalizes before its containment check, compares by
  path components after the macOS alias normalization the safety invariants
  require, and opens the path it checked.
- **Tests.**
  - An unpaired node's push and pull are refused.
  - A node that is still pairing is refused by remote_ops and byterange.
  - A symlink inside a source that points outside it is refused by byterange
    and by pull.
  - A dangling symlink at a push destination is refused.

**P1c. Refuse what destructive operations can't honor.**

- **Copy.** `files.copy` refuses non-`Physical` sources when `move_files` is
  set, and non-`Physical` destinations (`copy/preflight.rs:117-119,134-154`).
- **Create folder.** `files.createFolder` gets a validator. Its `items` go
  through the move validation, and `parent`'s device is respected
  (`create_folder/action.rs:87-94`).
- **Cross-device moves** are refused (D6).
- **Delete** routes each path by its variant: cloud paths to the cloud branch
  (`delete/strategy.rs:116-117,138-241`), local paths to the local branch.
  These go:
  - `RemoteDeleteStrategy`;
  - the unregistered `file_delete` protocol;
  - the deprecated `SdPath::device_id()`.
- **Empty trash.** `files.trash_empty` purges a recorded location only when
  its `Subject` still holds and the location lies inside a trash directory
  (`core/src/ops/files/trash.rs:82`). It gets a validator and a preview.
- **Track a source.** `sources.track` takes an `SdPath` and refuses another
  device's path. The explorer hides "Add to Library" for files on other
  devices, as it already does for Show in Finder. Routing the add to the owner
  belongs to `2026-09-16-add-to-library.md`.
- **Cloud.** `volumes.add_cloud` refuses a fingerprint that is already
  registered. `find_cloud_volume` releases the read lock, then takes the write
  lock with `.await`.
- **Tests** cover each refusal and a delete batch that mixes local and cloud
  paths.

### P2. Locators name places by UUID

Implements D1, D2 and D3. One type change and one regeneration. Once the type
lands, the remaining work splits across agents.

- **Type and codec.**
  - Add the target `SdPath`, `ContentRef`, the derived `Deserialize`, and the
    codec with its fixture. `SdPath::local` fills in the current device's
    UUID.
  - Remove:
    - `same_device` and the string forms in `is_current_device`;
    - `from_uri_with_context`, and `to_uri` alongside `display`;
    - `SdPath::new`, and `SdPathBatch::by_device` and `local_only`;
    - the deprecated accessors.
- **Core consumers.**
  - Slug comparisons become UUID equality in:
    - listing, search and thumbstrips;
    - copy routing and the move's source deletion;
    - event filtering (`core/src/infra/event/mod.rs:426,454,526,571`);
    - replica lookups (`core/src/service/mounts/peer.rs:59-79`,
      `core/src/ops/paths/reach.rs:178-194`, `arena_search.rs:69-74`);
    - the `device-<uuid>` fallback
      (`core/src/service/file_sharing.rs:211-216`).
  - `Library::resolve_device_slug` leaves the copy path.
  - Job dedup keys and `File::planned` ids follow the new display form.
- **Persisted values.**
  - A `library.db` migration rewrites `ItemType::Path` items from slug to UUID.
    It tries the library's devices first, then this device's own slug and
    `"local"`.
    - Items that resolve to nothing are deleted and logged.
    - `Content` and `Sidecar` items are deleted.
  - `get_layout` skips an item it cannot parse instead of failing the layout.
  - Jobs persisted in the old shape fail to resume, and startup reconciliation
    marks them failed with the reason.
  - Tab state, view preferences and sort preferences that no longer parse fall
    back to defaults.
- **CLI.**
  - File commands fill in the target device's UUID, taken from `core.status`
    through the `--device` forwarding.
  - Paths are canonicalized only when the target is this machine.
  - A path that doesn't exist yet resolves its parent and keeps its own name.
    This fixes `archive --to`, `link --at`, and copy to a new name.
- **Clients.** These move to device UUIDs:
  - the explorer, sidebar and inspector;
  - DnD pins, and the tab and preference stores;
  - `packages/ts-client` (`virtualFiles.ts`, `useNormalizedQuery.ts`,
    `subscriptionManager.ts`);
  - `apps/mobile` and `apps/native`.

  They read the current device through one hook over `devices.list`
  `is_current`, and format and parse addresses through the TS codec. The
  PathBar's URI mode uses the codec too.
- **Sidecar removal (D3).** The routes, command and frontend branches listed
  under D3.
- **Tests.**
  - JSON and rmp round-trips for every variant.
  - The URI fixture, in Rust and in TS.
  - Windows path operations on a Unix host.
  - The space-item migration, and a space item that fails to parse.
  - The CLI builds the target's UUID under `--device`.
  - The Rust-to-TS bridge tests pass the device id.

Acceptance: no production code compares a device slug or stores one inside an
address.

### P3. Durable targets

Depends on P2.

- **Types.** Add `SourcePath` and `RecordRef`. The resolver converts both ways
  and checks that the selected library holds the source.
- **Space items.** Space items gain `ItemType::SourcePath`. A pin beneath a
  source stores one, and the migration converts existing `Path` items beneath
  a source. `ItemType::Path` stays for literal paths outside any source.
- **Tags.**
  - `tags.apply` and `tags.unapply` take `RecordRef`s and open the named
    store, adapter stores included.
  - `File` gains `source_id`.
  - The store scan in `targets.rs` goes.
  - The outbox is already keyed by source.
- **Adapter records.** Adapter rows carry a `RecordRef`, and the sources view
  can tag them.
- **Processing policies.** The processing-policy target in
  `locations-research.md` uses `SourcePath`.
- **Tests.**
  - A pin beneath a source survives the drive remounting at another mount
    point.
  - A target that can't be resolved keeps its reason.
  - Tagging by `RecordRef` opens only the named store.
  - An adapter record takes a tag.

### P4. One resolver for execution

Depends on P2 and P3.

- **Every operation resolves through it.** That covers every operation in
  `core/src/ops/files`, listing, search scope, thumbnails, `files.stream_url`
  and `files.local_path`, which then report the resolver's codes.
  `PathResolver`, `PathResolutionError` and `resolve_in_job` go.
- **Scope** follows D4, through the session the resolver receives.
- **Expected versions.** Destructive inputs carry an optional expected version
  for each target:

  ```rust
  pub struct TargetPath {
  	pub path: SdPath,
  	pub expect: Option<Subject>,
  }
  ```

  - Delete, move sources, rename and attributes take `TargetPath`.
  - Validation compares at dispatch. The job compares again immediately before
    each mutation and skips with `address.stale` on a mismatch.
  - The explorer fills `expect` from the listing's size and modification time,
    compared at the precision the listing carries. The CLI leaves it empty.
  - The action stays the contract (`2026-09-22-action-previews.md:364-365`);
    nothing is leased.
- **Tests.**
  - Each code, from each kind of failure.
  - A delete whose file changed after the listing is skipped as stale.
  - A remote caller's permissions follow D4.

### P5. Content addressing

Depends on P1a and P4.

- **Accepted by reads.** `files.stream_url`, `files.local_path` and
  `thumbs.request` accept a `ContentRef`.
- **Resolution.** It finds holders in local stores and replicas by `uuid` or
  `candidate_uuid`.
  - A confirmed uuid reaches confirmed holders only.
  - A candidate uuid resolves when every holder agrees, and is ambiguous when
    holders carry different integrity hashes.
  - It prefers an online local holder, then a paired device's holder through
    the byte plane.
- **Shared lookup.** `files.alternate_instances` and `files.duplicates` use the
  same lookup.
- **Tests.**
  - A content reference resolves locally.
  - With the local copy offline, it resolves to a paired device's copy.
  - Two integrity classes under one candidate are ambiguous.
  - A confirmed uuid never reaches an unverified holder.

### P6. Cloud identity

Depends on P2 and D5.

- **Fingerprint inputs**, per service:
  - the S3 family: the endpoint (or the AWS partition when there is none) and
    the bucket;
  - Azure: the account name and the container;
  - GCS: the bucket;
  - Drive, OneDrive and Dropbox: the provider's account id and the root, read
    from each provider's current-account call when the storage is added.

  `input.service` must match the config variant
  (`add_cloud/action.rs:18-23,106-109`).
- **Stable ids.** Cloud volume ids are `v5` of the fingerprint, at add and at
  load, as local volumes already do (`core/src/domain/volume.rs:755`).
  Credentials key on the same fingerprint.
- **Indexer.** It carries `(volume_id, key)` instead of URI `PathBuf`s
  (`core/src/ops/indexing/job.rs:299-304`, `discovery.rs`).
- **Refresh.** The periodic refresh stops removing registered cloud volumes
  (`manager.rs:898-935`).
- **Tests.**
  - Two accounts with default roots stay two volumes with two credentials.
  - One bucket name on AWS and on R2 stays two volumes.
  - Indexing reaches a subdirectory's children.

## Boundaries

- **Non-UTF-8 names are out of scope.** The walk records them lossily
  (`core/src/volume/backend/local.rs:221-232`), and the store has no column for
  the raw bytes. An address can carry them only once the store can.
- **Journal effects** keep device-local absolute paths. Undo already refuses
  when a `Subject` no longer holds.
- **Library sync** stays deferred. `SourcePath` and `RecordRef` are the values
  it will carry.
- **Scoped grants** for sharing with other people belong to
  `docs/core/design/shares-and-remote-libraries.md`.
- **Cloud as a source adapter.** Whether cloud storage becomes one belongs to
  `docs/core/cloud-integration.mdx`. D5 holds either way.
