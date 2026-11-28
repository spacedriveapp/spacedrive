# Spaces

Status: design, adopted 2026-11-27. Built in stages; see PROJECT_STATUS.md.

A space is a folder, or a new empty directory, that Spacedrive treats as one unit of work. A space has an owner device, a set of actors who change it, a log of those changes, and a policy for indexing, protection and history. Spaces are how you hand a folder to an agent, mount it on another machine, and see who changed what.

This document defines spaces, what they add to the write path, how history and merging work, and how they are exposed. It builds on [agent-grants.md](agent-grants.md), [agent-writes.md](agent-writes.md), [agent-access-threat-model.md](agent-access-threat-model.md), [mounts.md](mounts.md) and [source-durability.md](source-durability.md).

## Principles

1. **One authority per space at a time.** A space has one node that accepts writes. Mirrors and hosted replicas work by moving or leasing that authority. Two writers never converge blindly.
2. **Refuse, never overwrite.** A stale write is refused. A create that collides keeps both items. This is the floor for every surface, including mounts and replicas.
3. **Every change has an actor.** A person, a device or a named agent. When Spacedrive cannot tell, it says "changed outside Spacedrive on <device>" and never shows nothing.
4. **History is opt-in per space.** Ordinary indexed folders stay cheap. Turning versioning on buys per-file history, snapshots, forks and merges for that space only.
5. **A replica can stand in, but is never required.** An always-on node the person runs (a NAS or a VPS) can serve a space while the home device is off. The home device keeps the custody line in `product-direction.mdx`.
6. **Network exposure follows the threat model.** Network surfaces are designed and built with spaces, but ship switched off behind a config flag until an outside security review passes. Nothing listens beyond loopback until then.

## Naming

The sidebar type that is called `Space` today is renamed `Layout`. The word "space" now means the unit in this document.

- `core/src/domain/space.rs`, `core/src/ops/spaces/*` and the interface switcher use `Layout` (`SidebarLayout` in code, "Layouts" in the switcher).
- The `space`, `space_group` and `space_item` tables are renamed by migration. Data is kept.
- Ops move from `spaces.*` to `layouts.*`. The `spaces.*` prefix is then reused for the new entity. No alias is kept, because the beta has no external API consumers.

Boards stay a separate thing. A board groups media across folders. A space is a folder.

## Data model

### Where spaces live

Spaces are stored in the source store (`data.db`) as an assertion table, not in `library.db`. Each row is stamped with an HLC and the origin device. Concurrent edits resolve by last writer wins. A delete is a tombstone, so it propagates. Spaces sync as assertions, the same path tags use, and not through the frozen `Syncable` framework.

### space

```
space
  id               uuid
  name             text
  kind             folder | workspace
  source_id        uuid              # the source that holds it
  root             text              # canonical path inside that source ("" = source root)
  home_device      uuid              # device that owns the bytes today
  authority        device:<id> | node:<id> | bucket:<source_id>
  authority_epoch  int               # bumped on every hand-over
  versioning       off | on          # off by default
  content_indexing on | off          # hashing and text extraction for files in the space
  protected        json              # extra protected patterns beyond the built-in list
  created_by       actor_id
  created_at, updated_at
```

A space is a pointer at a directory plus policy. Creating a `folder` space on an existing directory changes nothing on disk.

Creating a `workspace` space makes an empty directory under the workspaces root and tracks it as a source if no source covers it. The default root is `<data dir>/spaces/<id>/`. A person can choose another folder.

The folder can be a prefix of a connected bucket, `s3://bucket/prefix`. A bucket `folder` space needs at least one object under the prefix. A bucket `workspace` space needs an empty prefix and writes a directory marker object for it. The prefix is tracked as a bucket source when no source covers it. `content_indexing` is off for bucket spaces and cannot be turned on, because reading contents would download every object.

### Content indexing replaces deep scope

Content indexing is a property of a space. It replaces the per-source deep scope from #3286, where subtrees of a source opted in to hashing and text extraction. There is one system for subtrees with policy, and it is spaces. The home source's standard user folders (Documents, Desktop, Downloads, and similar) appear as spaces with content indexing on. Files outside any space with content indexing on are not hashed or text-indexed by default, so a home source does not grow a large text database.

### space_replica

```
space_replica
  space_id, replica_id
  location     device:<id> | node:<id> | bucket:<source_id>
  role         authority | replica | mirror      # mirror = editable copy
  mode         full | on_demand                  # whole copy, or cache on read
  state        syncing | current | stale | offline
  applied_seq  int                               # last event seq applied
```

### actor

```
actor
  id          uuid
  kind        person | device | agent
  label       text                  # "Jamie", "Mac Studio", "research-agent"
  harness     text?                 # "claude-code", "codex", custom
  public_key  bytes?                # for agents that hold a standing key
  created_by  actor_id
```

Every grant points at an actor. Several grants, one per task, can share one agent identity, so the history of that agent stays together.

### grant

`agent_grant` gains these columns:

- `space_id`: nullable for old rows, required for new ones. Scope becomes a subtree of a space.
- `actor_id`: the actor the grant acts as.
- `kind`: `task` or `standing`.
- `token_format`: `opaque` or `signed`.

A `task` grant lasts 5 minutes to 12 hours. A `standing` grant is for an agent's own workspace, lasts up to 30 days, and is allowed only on `workspace` spaces.

Grants sync as assertions, so any replica can honor them. A signed token is Ed25519 over `{grant_id, space_id, subtree, caps, exp, epoch}`. A replica verifies it without a lookup. Revocation is a synced assertion checked on every request.

### space_event

The change log for a space. It is separate from `grant_audit`, which stays the bounded request log for security review.

```
space_event
  seq          int               # monotonic per space, assigned by the authority
  space_id
  at
  actor_id
  grant_id?
  origin       agent_write | app_op | watcher | replica_apply | merge | restore
  op           create | modify | move | delete | mkdir | restore | snapshot | fork | merge
  path, to_path?
  before_hash?, after_hash?      # BLAKE3, the same hashes preconditions use
  size?
  version_id?                    # when versioning is on
  undo_id?                       # the files.undo job id
  authority_epoch
```

The log lives at `sources/<source id>/spaces/<space id>/events.db`, so it travels with the space, replicates by seq, and can be exported.

The log covers only paths inside a space. Watcher ingest for a whole source never writes an event or a job per change. A source with no space has no event log.

Retention is 30 days by default, configurable per space. With versioning off, a space keeps 30 days of metadata-only events. With versioning on, versions and their events are kept for 30 days, except versions pinned by a snapshot or by a fork's base.

### Attribution

- App and op-registry actions are attributed to the person actor of the device.
- Agent writes, and writes through a Spacedrive mount, carry the actor of the grant.
- Changes seen only by the watcher are attributed to the home device with origin `watcher`. The interface shows them as "changed outside Spacedrive on <device>". This is enough to answer whether you or an agent made a change.

### Versioning objects

These exist only for spaces with versioning on. They live in the space's directory, beside its event log: `versions.db` and `blobs/`. [space-versioning.mdx](../space-versioning.mdx) describes the built behavior.

```
chunk     a FastCDC chunk (about 64 KiB average), stored once in blobs/ under its BLAKE3 hash
blob      hash (BLAKE3 of the whole file, the precondition hash), size, ordered chunk list
version   id, path, blob_hash? (none for a delete), size, mode, mtime, at, actor_id, origin, event_seq?
snapshot  id, name, manifest_id, created_by, at                  # a pinned manifest
fork      child space_id, parent space_id, base manifest_id       # not built yet
```

The `version` table is the manifest kept as a delta log. A row says what one path holds from that row on, and its id is a manifest id. The manifest at an id is the newest row per path at or below it, and per-file history is the rows for one path.

Captures follow the event log. An agent write, a restore or an app operation is captured inside the same write turn as its event, from the bytes it just committed. A change seen by the watcher is captured once the path has been quiet for 1.5 seconds, so a burst of saves is one version. A change that was overwritten before that capture read it is lost, and the interface says so. Turning versioning on captures the files already in the space as a baseline. Turning it off stops capture and keeps the versions and snapshots, which age out under retention; `spaces.history.purge` deletes them. Chunking keeps versioning of large media affordable when only part of a file changes.

Retention keeps every version newer than 30 days, the newest version of each path, and every version a snapshot's manifest needs. Blobs and chunks that no kept version reaches are then deleted.

A snapshot is a named manifest. It costs O(entries) and copies no bytes. A fork is a new space whose root is materialized from a manifest, using reflinks where the volume supports them and independent copies otherwise. Hard links are never used, because an editor that writes a file in place would change the parent's bytes too. A fork can also be an on-demand space served from blobs. Restoring a file or a whole space goes through the normal write path, so a restore is attributed and undoable.

### Portable space file

`sd space export <space> out.sdspace` writes one SQLite file with the space record, events, manifests and the referenced blobs. `--current` writes only the blobs of the current manifest. `sd space import` reverses it. This is also the format a backup of a space uses.

## Write path changes

The agent write path (`core/src/ops/agent_write/`) becomes the single write path for spaces. MCP, the HTTP API, mounts, restore and replica apply all use it.

- **Per-file preconditions.** An agent write may carry a per-file `after_hash` precondition, the hash of the path's last event. A precondition does not go stale when unrelated files in the source change. `content_hash` and the store `revision` keep working as they do today.
- **Authority epoch fencing.** A space carries `authority_epoch`. Every write names the epoch. A node that is not the current authority refuses the write and points to the authority. For now a space has a single authority node.
- **Event before commit.** The `space_event` row, and the blobs when versioning is on, are written in the same critical section as the journal row.
- **Protected patterns per space.** The built-in names in `grants/scope.rs` are joined by the default patterns in `grants/protect.rs`: `GEMINI.md`, `AGENT.md`, `.cursor/rules/**`, `.github/workflows/**` and `.env*`. Every space holding a path adds its own `protected` list. Names are folded the same way as the built-in names. `docs/core/design/agent-grants.md` has the matching rules.
- **Conditional writes on buckets.** `CloudBackend::write` gains `If-Match` and `If-None-Match` where the service supports them, so bucket-backed spaces refuse stale writes. Services without conditional writes are read-only for agents.

## Hosted replicas

A space can keep a replica on an always-on node the person runs, such as a NAS or a VPS. The replica serves reads and, when allowed, writes while the home device is off.

1. Enabling a replica creates a `full` replica. The home device sends blobs and streams `space_event` rows by seq. While the home device is online it stays the authority. The replica forwards writes to it and serves reads from its copy.
2. The replica takes authority when the home device has been unreachable for a grace period and the space allows it. It acquires a lease by bumping `authority_epoch` in a record both sides can see. The always-on node is the lease arbiter for its spaces. The lease is short and the authority must renew it. A home device that cannot renew before the lease expires stops committing writes (fail closed), so the replica takes over only after the old lease has run out. The grace period and lease length are set by the implementer.
3. When the home device reconnects, it sees the higher epoch, stays out of the authority role, and pulls events from its `applied_seq`. It applies each through the write path with the event's `before_hash` as precondition.
4. Changes made on the home device while it was offline are found by the startup reconcile and compared with the pulled events. For different files, both sets apply. For the same file with versioning off, the home device's version is kept under a conflict name and both appear in the activity feed. For the same file with versioning on, a three-way text merge runs against the last common version, and a failed merge falls back to conflict copies.
5. Authority returns to the home device, with a new epoch bump, once it has caught up.

A replica is another node. It is never required.

## Editable mirrors

A mirror is a `space_replica` with role `mirror` on another device, such as a VM. It is not a second authority.

1. **Write-forwarding mirror.** The VM mounts the space in `full` or `on_demand` mode. Reads are local. Writes go through the write path to the current authority with preconditions. A stale write lands as a conflict copy on the mirror and is reported. This needs reachability but no new consistency model.
2. **Lease-holding mirror (later).** The VM takes the lease for the length of a task, works offline, and hands authority back with the same reconcile as a hosted replica.

To give a VM an editable copy of a folder, you create a grant on the space, copy one command, and run `sd mount <url> --token-file <path> <dir>` on the VM, with the token in a file only that user can read.

## Mount and MCP surfaces

### Space endpoint

The space endpoint is the network face of a node. It carries MCP, a REST API that mirrors the MCP tools, and a block-read protocol for mounts. Transport is HTTPS, or QUIC through the existing Iroh stack. Every request authenticates with a grant token and goes through `authorize`.

The endpoint, relay and remote mount ship switched off behind a config flag. They stay off until an outside security review passes. Local agent access is read-write from the first release.

Reachability options:

1. **Relay.** The home node keeps an outbound connection to a relay that gives it a stable URL. TLS terminates at the node, not the relay. The relay can be self-hosted.
2. **Direct address.** For a LAN or a private network, an opt-in `--listen` with TLS, once the M1 checklist passes.
3. **Replica.** For spaces with a replica, the always-on node answers directly.

### Thin mount client

`sd mount <url> --token-file <path> <dir>` is a small client in `sd-cli` with no library and no pairing. It mounts one granted space. The token comes from a file readable only by its owner, or from standard input with `--token-file -`. No command takes a token as an argument, since arguments are visible to other users through `ps` and persist in shell history.

- On Linux it uses FUSE, reusing `core/src/service/mounts/fuse/` and the block cache.
- On macOS it runs a localhost NFSv3 server and mounts it with `mount_nfs`. This needs no kernel extension. FSKit stays the long-term destination.
- Windows comes later.
- Reads fetch 1 MB blocks on demand, cache them as local mounts do, and read ahead.
- Writes are write-back on close. On open the client records the file's `after_hash`. On close, or after 2 seconds of quiet, it uploads the new bytes as a write with that hash as precondition. A stale write keeps the local bytes as a conflict copy in the mount and logs it. Renames and deletes are immediate writes with preconditions. `fsync` waits for the upload.
- The client subscribes to the space's event stream and invalidates cached blocks and listings by path.

### MCP and REST

The existing tool set and limits stay. New tools are `list_spaces`, `list_changes` (the event log since a seq, so an agent can catch up), `history` and `restore` (versioning on), `snapshot`, `fork`, and `search_semantic`. REST mirrors the same operations under `/v1/spaces/<id>/...` with the same token. One tool registry generates both, so they cannot drift.

## Version control and merge model

| Option | What it gives | Cost | Fit |
|---|---|---|---|
| Refuse stale writes, keep conflict copies | Never loses data. Already built. | You resolve conflicts by hand. | The default for everything, including binaries and media. |
| CRDTs for text | Live co-editing | Every editor must speak the CRDT. Files arrive as whole-file writes, so a CRDT reduces to a text diff of two snapshots. | Only for a native document editor. |
| Three-way merge on text | Edits to different parts of a file both survive | Needs a common base, which versioning provides. Wrong for binaries. | Good where two lines of work meet. |
| Snapshots and forks | Isolated parallel work you can compare and merge | Storage for manifests and blobs | Several agents working on one folder at once. |

The model combines three of these. There are no CRDTs.

- **At write time**, on every surface, preconditions refuse stale writes and creates keep both. Two writers never interleave inside one file.
- **With versioning on**, every committed change is a version, and snapshots and forks are manifests. Each parallel agent gets its own fork. Two agents never share one fork.
- **At merge points only**, such as merging a fork back or a replica or mirror handing authority back, each file is merged on its own. If only one side changed it, that side wins. If both changed a text file (UTF-8, under 8 MiB, not a known lockfile or minified asset), a line-based three-way merge runs against the common base. A clean result is applied as an event with origin `merge`. Otherwise, and for every binary file, the merging side's version is kept under a conflict name and reported. A delete against a modification keeps the modification. Spacedrive never writes conflict markers into a file unless the person asks for them on a code space.

Format-aware merges, such as CSV rows by key or JSON, can plug into the same interface later.

Two people editing the same paragraph at the same moment get a stale refusal or a conflict copy. This is the honest model for a filesystem that Finder, editors and agents all write to.

## Search scoped by space

- **Content indexing is per space.** Hashing and text extraction run for spaces with content indexing on. See the data model above.
- **Semantic search.** The existing text embedder is wired to the text index. Spacedrive chunks the extracted text and the transcript, caption and scene sidecars, embeds each chunk, and stores vectors in a per-source `vectors.db`. It runs only for spaces with semantic search on, which bounds cost. `search.semantic` and `search_semantic` expose it, with hybrid ranking against FTS5 BM25.
- **Search runs where the bytes are.** All search tools run on the node that holds the files. A thin mount client never downloads files to search them. The agent skill tells agents to prefer the `grep` tool over `grep -r` on a mount.

## Security model

**Identities.** People and devices work as today, through pairing and node keys. Agents are `actor` rows. Credentials are grants: opaque `sdg_` tokens first, then signed tokens that any replica can verify.

**Enforcement.** One check, `authorize`, takes `(token, space, path, cap, epoch)` on every surface: MCP, REST, the mount protocol and replica apply. It keeps today's resolution by filesystem identity and protected names.

**Gates**, from [agent-access-threat-model.md](agent-access-threat-model.md):

1. Before opt-in network listening, all 15 M1 checklist items pass. They include TLS and handle-based path containment: open files relative to an `O_PATH` or `openat2(RESOLVE_BENEATH)` handle on the grant root on Linux, `O_NOFOLLOW_ANY` on macOS, and handle-relative opens on Windows. The raw RPC line logging (F2) is fixed.
2. Before default exposure, relay serving or writable grants over the network, an outside security review covers the verifier, containment on all three platforms, the endpoint and its limits. F1 (a per-daemon RPC secret) and F5 (`sd-server` binds loopback by default and rate-limits failed logins) are fixed.
3. F3 (byterange follows symlinks) is fixed before byterange carries anything a grant scopes.

Network agent access is built for reads and writes and stays switched off behind a config flag until gate 2 passes. Turning it on is a release step, not new design work.

**What a grant does not do.** An agent on the same OS account with a shell is not contained by a grant. Sandboxed agents on another machine are the case grants solve.

## Editable mirrors and the write path

Mirrors, replicas and mounts all write through the single write path above. None of them has a private route to the store. A mirror's write is an agent-style write with a precondition, so it is refused when stale, attributed to its actor, and logged as a space event.
