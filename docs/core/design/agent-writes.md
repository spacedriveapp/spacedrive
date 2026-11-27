# Agent writes

> **Amendment (2026-11-27):** [spaces.md](spaces.md) supersedes this document in part. Where it says that writes from more than one device, merging, or the sync freeze are out of scope, spaces.md now governs: each space has one authority, handed over by lease and epoch fencing, merges happen only at merge points (a fork merged back, or a replica or mirror handing authority back), and spaces sync as assertions, not through the frozen sync framework. Three behaviors here also change. Writes may carry a per-file `after_hash` precondition. Protected patterns are extended and merged with a per-space list. A space event is written in the same critical section as the journal row.

An agent that holds a writable grant can create, change, move and delete
files inside the granted directory. This document defines how those writes
behave: where they run, what each one must name, what happens on a
collision, how they are undone, and what they refuse. The grant model itself
is in [agent-grants.md](agent-grants.md).

## Model

Writes are single-owner and write-through.

- **Single owner.** A write runs on the device that owns the source, the
  same device where the grant's directory resolves. The source's watcher
  ingests the change like any other change on disk. No second path updates
  the store.
- **Preconditions.** A write that changes an existing path names what the
  agent read. If the path no longer matches, the write is refused as stale.
  The refusal carries the path's current state.
- **Keep both.** A write that creates a path never replaces anything. If the
  name is taken, the new item gets a conflict name and both are kept.
- **No merging.** Spacedrive never combines two versions of a file. A stale
  write is refused, and the agent decides what to do after it reads again.

Writes from several devices to one source are out of scope. So are merges
of any kind, row-level merges of CSV or similar files, writes to adapter
sources other than buckets, and writes to a source that a peer device owns.
Bucket sources take writes under their own rules, in "Bucket sources" below.

## Operations

| Operation | Wire name | Precondition | When the target name is taken |
|---|---|---|---|
| Write a whole file | `agent.write_file` without `offset` | Optional. Absent means create. | Create: conflict name. Change: refused as stale if it does not match. |
| Write a byte range | `agent.write_file` with `offset` | Required | Not applicable. The file must exist. |
| Make a directory | `agent.make_dir` | None | An existing directory is the answer, unchanged. Anything else: conflict name. |
| Move or rename | `agent.move` | Required, checked at `from` | Conflict name at `to` |
| Delete | `agent.delete` | Required | Not applicable |
| Read state | `agent.stat` | None | Not applicable |

Every operation takes the grant token and paths relative to the granted
directory. The four writes need a `read_write` grant. `agent.stat` works
with any grant.

A byte-range write lays `content` over the existing file starting at
`offset` and keeps every other byte. An offset at the end of the file
appends. An offset past the end is refused.

Content is UTF-8 text by default. Set `encoding` to `base64` for binary
content. One write carries at most 64 MiB after decoding.

A write creates any missing parent directories. Each one is journaled, so
undo removes it again.

Moves stay on one volume. A move that crosses volumes is refused.

## Preconditions

A precondition has two optional fields. A write that changes an existing
path must name at least one, and every field it names must match.

```json
{
  "revision": "6f1c...e2:418",
  "content_hash": "9f86d081884c7d65..."
}
```

- `revision` is the revision of the store that ingests the path, written as
  `store_id:value`. The value counts every change the store has committed
  anywhere in the source, so it is strict: any ingested change in the source
  makes it stale. It reflects the disk as of the last ingest.
- `content_hash` is the BLAKE3 hash of the path's contents, as lowercase hex.
  Spacedrive computes it from the disk at check time, so it catches changes
  the store has not ingested yet. For a directory it is the hash of the
  directory's listing: each child's name, kind, size and modification time,
  sorted by name. A directory hash changes when a child is added, removed,
  renamed or rewritten. For a symlink it is the hash of the link target.

`agent.stat` returns both values. Every write returns them for its result in
`state`, so an agent can chain writes to one file by passing the returned
`content_hash`. A revision returned by a write is the revision before the
watcher ingests that write, so use the content hash to chain writes.

A file that is a cloud placeholder, with contents not on this device, has no
content hash. Spacedrive never downloads it to compute one. A precondition
on such a file can only name the revision, and a byte-range write to it is
refused.

### Stale refusals

A refused write returns an error that starts with `stale:` and names the
path's current state, including its kind, revision and content hash, or
`absent` if nothing is there. Read the path again, decide, and retry with
the new values.

The precondition is checked twice. It is checked once when the request
arrives. Just before the rename that commits the write, Spacedrive checks
that the target still matches what it checked, and that the path still
resolves inside the grant. A file is compared on size and modification
time. A directory's listing is hashed again. A change between the two
checks is refused as stale.

## Conflict names

When a create, a make-directory or a move finds its target name taken, the
item lands beside it under the first free name of this form:

| Target | Conflict name |
|---|---|
| `notes.md` | `notes (agent conflict 1).md` |
| `archive.tar.gz` | `archive.tar (agent conflict 1).gz` |
| `.env` | `.env (agent conflict 1)` |
| `Makefile` | `Makefile (agent conflict 1)` |
| directory `v1.2` | `v1.2 (agent conflict 1)` |

The number counts up from 1 until a name is free. A file keeps its last
extension so it opens with the same application. The result's `conflict`
field is `true`, `requested` holds the path the agent asked for, and
`state.path` holds where the item landed.

## Staging and commit

New bytes go to a hidden staging file beside the target, named
`.sd-<id>.sdpart`. This is the same staging the peer transfer receive uses.
The staging file is flushed to disk, then renamed to the target. A reader
sees the old file or the new one, never a partial one. A staging file left
by a killed process is listed under `receiving/` in the data directory and
removed at the next start.

The rename never replaces an existing name. On Linux it uses
`renameat2(RENAME_NOREPLACE)` and on macOS `renamex_np(RENAME_EXCL)`. A
filesystem that does not support those calls, and other platforms, check for
the name and then rename. If the name is taken at commit time, the write
takes the next conflict name.

When a write changes an existing file, the previous bytes go to the trash
first and the staged file is renamed into the freed name. The two renames
leave a short window in which the path is absent. If the second rename
fails, the previous bytes are put back from the trash. The original's mode
is applied after the rename.

All agent writes on one daemon take one lock from the grant check to the
journal row. Two agents on the same daemon cannot interleave a check with a
commit. Writers outside Spacedrive are caught by the second check.

## Undo

Each write gets an id, and every change it makes is recorded under that id
in the job journal that `files.undo` reads. The write returns the id as
`undo`. Pass it to `files.undo` as `job`.

| Write | Journal effect | Undo |
|---|---|---|
| Create a file or directory | `created` | The item goes to the trash. |
| Change a file | `replaced`, with the trashed previous bytes | The new file goes to the trash and the previous bytes return. |
| Move | `moved` | The item moves back. |
| Delete | `trashed`, with the trash location | The item returns from the trash. |
| Missing parent directories | `created` for each | Each goes to the trash. |

Undo checks each effect's result before it reverses it. If a later write,
by an agent or anyone else, changed the result, undo leaves it alone and
reports it. A write that changed nothing, such as making a directory that
exists, returns no undo id.

Delete never removes anything permanently. It uses the platform trash where
the volume has one and otherwise a Spacedrive trash at the volume root,
`.spacedrive/trash/<id>/`.

## What is refused

The grant's `authorize` check runs on every path a write touches, the
source and the destination of a move included. It refuses:

- tokens that are unknown, expired or revoked, and grants whose source was
  removed;
- `read` grants;
- paths outside the granted directory, including `..`, absolute paths
  elsewhere, and symlinks that point out;
- instruction files and git internals: any `AGENTS.md`, `CLAUDE.md` or
  `SKILL.md`, and `.git` at any depth, in every spelling the grant check
  recognizes.

Authorizing a directory does not authorize what is in it. A move or delete
of a directory lists its whole subtree, without following links, and is
refused if any path in it is protected.

The writes also refuse the granted directory itself as a target, a move into
the item's own subtree, and a byte-range write to a cloud placeholder.

A hard link to a protected file under another name does not let an agent
change the protected file. Writes never modify a file in place: they
replace the name with a new file, so the other name keeps the old bytes.

## Bucket sources

A grant on a prefix of an S3 or R2 bucket source takes the same write
operations with the same answers, and the bucket enforces the precondition
itself. Code: `core/src/ops/agent_write/bucket.rs`.

- **Change.** The write reads the object's entity tag, streams the bytes at
  that tag into a BLAKE3 hash, and checks the precondition against it. It
  then sends the new bytes with `If-Match` and that tag. An object that
  changed or was deleted after the check, by an agent or by anything outside
  Spacedrive, makes the service refuse the write, and the agent gets the
  usual stale refusal with the current hash. No lock is involved.
- **Size.** Hashing holds one chunk in memory, whatever the object's size.
  A byte-range write sends the whole object back, so it takes objects up to
  64 MiB and refuses larger ones.
- **Create.** The write is sent with `If-None-Match: *`. A taken key is
  refused by the service, and the write tries the next conflict name, so
  two creates racing for one name both keep their bytes.
- **Revision.** A bucket source has no store revision yet. `revision` is
  absent from its state, and a precondition that names one is stale. Use
  the content hash.
- **Directories.** A prefix with objects under it is a directory. Its hash
  is the listing hash, as for a local directory. `make_dir` writes a
  zero-byte `prefix/` marker.
- **Refused.** Moves and deletes, because a bucket cannot make them
  conditional on the version the agent read. Undo, because a bucket has no
  trash; the provider's object versioning is what restores a version.
- **Read-only services.** A service whose backend cannot refuse a stale
  write, such as Google Drive, OneDrive or Dropbox, is read-only for agents.
  `grants.create` refuses a `read_write` grant on it, and `authorize`
  refuses every write request with a message that says why.

`core/tests/bucket_agent_acceptance_test.rs` runs these cases against a
real S3-compatible server.

## Acceptance matrix

`core/tests/agent_writes_acceptance_test.rs` covers each row.

| Case | Expected |
|---|---|
| Write with the current content hash | Lands. The result carries the new hash and an undo id. |
| Write with an old content hash | Refused as stale. The error carries the current hash and revision. The file is unchanged. |
| Write with the current revision | Lands. |
| Write with any other revision | Refused as stale. |
| Current hash with an old revision | Refused as stale. |
| Change with an empty precondition | Refused: precondition required. |
| Byte range at an offset inside or at the end of the file | Lands, other bytes kept. |
| Byte range past the end, or without a precondition | Refused as invalid. |
| Create at a free name | Lands with parents created. `conflict` is false. |
| Create at a taken name, twice | Both files kept: `(agent conflict 1)`, then `(agent conflict 2)`. |
| Base64 content | Written as bytes. |
| Make an existing directory | Unchanged, no undo id. |
| Make a directory where a file is | Directory made under a conflict name. |
| Move with an old precondition | Refused as stale. |
| Move to a taken name | Moved under a conflict name. The target is kept. |
| Move a directory with its listing hash | Lands. |
| Move a directory into itself | Refused. |
| Delete with an old precondition | Refused as stale. The item is kept. |
| Delete with a current precondition | Item goes to the trash. |
| Undo a delete, a move, a create, a change | Each reversed. |
| Undo a change that a later write built on | Left alone and reported. |
| Write, move, delete or make-directory on `AGENTS.md`, `CLAUDE.md` or `.git` | Refused as protected. Nothing changes. |
| Move or delete a directory that holds `AGENTS.md` or `.git` | Refused as protected. Nothing changes. |
| Change a read-only file | Lands. The file keeps its mode. |
| Write through `..` | Refused as an escape. |
| Write with a read grant | Refused as read-only. |
| Any run | No staging file is left in the grant. |
