# Storage Consolidation and the Four Contracts

Captured 2026-08-18, before any slice 2 wiring. Source: an external architecture
review (GPT-5.6, 2026-08-18) pressure-testing the written design against the
shipped slice 1, cross-checked against this repo's own audits — several findings
were independently confirmed by both. The review's verdict, accepted here: the
direction is right, the schema simplifications hold, but the coordination layer
accreted one store per document until the steady state became a small
distributed system running on one machine. This doc collapses the storage
model and writes down the four contracts slice 2 must build against.

Amends: `2026-08-17-record-spine-transplant.md` (P2, P3, workstream B),
`docs/core/design/cross-location-content.md`, `docs/core/design/zero-onboarding-startup.md`,
`docs/core/design/ephemeral-snapshot-format.md`. Each carries a pointer here.

## The finding

The pre-consolidation steady state held arena state, ephemeral snapshots,
per-source `index.db`, per-source `meta.db`, `library.db`, `content-map.db`,
`global-index.db`, and sidecar stores — with independent consistency between
them. One filesystem mutation could touch five representations, each
asynchronously, which demands generations, replay, tombstones, and
reconciliation machinery *per pair*. Every store was locally reasonable; the
combination was not. Detachable drives make the system genuinely distributed,
so the coordination that remains must be deliberate, not accumulated.

## The collapsed model

Four storage concepts. Nothing else holds truth.

1. **`library.db`** — unchanged scope: source registry, settings, space
   definitions, genuinely cross-source assertions. Tag definitions moved out to
   the sources that use them; see `docs/core/design/tags-and-assertions.md`,
   which amends this entry and contract 2's treatment of tags.
2. **One `source.db` per physical filesystem** — the record spine, the identity
   ledger (contract 2), durable knowledge (tags, corrections, notes), facets,
   and per-source FTS, in **one transaction boundary**. The former
   `index.db`/`meta.db` split becomes a *logical* split inside this file:
   rebuildable tables (records, facets, FTS) carry a generation and are
   replaced wholesale on reindex; durable tables (ledger, assertions) are
   never touched by reindex. "Delete the index" means bumping the rebuildable
   generation, not deleting a file. What the split was buying — tags surviving
   index rebuild — the generation rule buys inside one transactional store,
   and the identity-rebind dance between two files disappears.
   *Retained from the split's one real virtue:* durable tables get a periodic
   `VACUUM INTO` export beside the store, because on hostile filesystems
   (exFAT, SMB) the merged file now shares corruption fate between churn-heavy
   index writes and irreplaceable assertions.
3. **One derived `catalog.db`** — the merge of the planned `content-map.db` and
   `global-index.db`: global record enumeration, detached display data, global
   name/ancestor FTS, and content-placement rows, as tables in one projection
   database. Rebuildable by sweeping source stores; deletable. Splitting it
   for write contention is a measurement-justified optimization for later, not
   a starting posture.
4. **Sidecars** — unchanged: content-addressed served artifacts, not
   relational state.

The arena and its snapshot are **caches**. They make the UI immediate and the
launch warm; they are never an authority and never an event source.

## Artifact classes and the quarantine rule

"Rebuildable" was doing dishonest work: a detached drive's data cannot
participate in a rebuild. Three classes, replacing the cache/data binary:

- **Authoritative** — `source.db` durable tables, sidecar files.
- **Rebuildable while attached** — rebuildable generations, `catalog.db`,
  arena, snapshots *of attached sources*.
- **Availability-bearing replicas** — anything that is the only local copy of
  a detached source's state: its snapshot, its `catalog.db` rows, its detached
  display data. Deleting one temporarily revokes a promised product
  capability.

Policy consequences: a decode failure or version mismatch on a **detached**
source's snapshot quarantines the file (`.snapshot.bad`) instead of deleting
it — a later decoder fix can recover it; delete-and-reindex applies only when
the root is present. `catalog.db` retains the previous-good generation of a
detached source's rows until a replacement generation is published. And
projections carry **trust levels**: eventually-consistent rows may power
badges and browsing, but destructive decisions — skipping a copy because
content "exists elsewhere," reporting something as backed up, garbage-collecting
a sidecar — require authoritative verification or fail conservatively. A
missing catalog row is never proof of orphanhood.

## The freshness posture, stated once

No routine full rescans. Watchers maintain the index; bounded reconciliation
repairs lost continuity; recovery scans are part of the architecture, not a
failure of it — filesystem watchers are not authoritative and nothing may be
built as if they were.

## Contract 1 — Source ownership

One physical filesystem identity owns one source id and one record namespace.
A Location is a subtree view carrying policy (durability, enrichment) inside a
source — never an indexing root, never a second namespace (transplant plan,
"What a Location becomes"). The source registry rejects a root nested under an
existing source's root. Desktop and Downloads are durable subtrees of the
system-volume source; any doc or example modeling them as sources is
superseded. Consequence: nothing is ever migrated twice — a subtree promoted
to durable is a policy row, not a new store.

## Contract 2 — Identity and rebinding

Three concepts, never conflated:

- **`source_id`** — the physical filesystem, established by the volume
  fingerprint ladder. The record stores *all* candidate fingerprints computed
  at attach (dotfile uuid, filesystem uuid, coarse metadata), and matching
  accepts any rung, upgrading to the strongest — a drive first seen read-only
  (no dotfile) must not become a second source when the dotfile lands.
- **`record_uuid`** — v7, minted at first observation, the permanent domain
  identity. The UI shows it, edges reference it, the spine persists it.
- **External evidence** — not one magic string: a versioned tuple
  (schema v1: relative path, inode, size, mtime) recorded per observation.
  Rebinding is evidence matching in the ledger, not key equality.

The **identity ledger** is a durable table in `source.db`: record uuid,
evidence history, first/last seen, tombstone. It exists because tags key on
record uuids and must survive index rebuild — a rebuildable table may never be
the only association between filesystem evidence and a uuid.

Rebind semantics: rename or move within a source (same inode, evidence
otherwise agreeing) rebinds — uuid preserved. Replace-at-same-path (new inode,
disagreeing evidence) is a new record; the old one is tombstoned — content
identity may still match, which is the catalog's business, not the ledger's.
Hardlinks are one record per path sharing a content identity; allocated-size
attribution follows the content-map policy. Clones (new inode) are new
records. Against inode reuse: rebinding requires agreement of at least two
evidence classes (path; inode; size+mtime) — no single-factor rebind.

## Contract 3 — Drain sequencing

The arena is not the durable event source. Per source, the **connector**
(walk + watcher, unified) produces one observation stream and fans out to two
consumers: the arena (immediate display, lossy-tolerant) and `source.db`
(durable, transactional). Two invariants survive from the original "drains
from the arena" phrasing, and they are the point:

1. **One observation stream.** There is no second walk, ever. Durability is a
   consumer of the same observations the hot layer consumes.
2. **Identity is minted at first sight, before fan-out.** The producer assigns
   (or ledger-resolves) the record uuid; both consumers receive it. The uuid
   in the UI and the uuid in the spine are the same because they were never
   two.

Delivery is at-least-once with: a per-source **generation** (bumped on
overflow bankruptcy, reindex, or continuity loss), a monotonic **sequence**
within the generation, snapshot handoff as (generation, high-watermark, full
enumeration), replay above the watermark, **idempotent** mutation application,
and the consumer's (generation, applied-watermark) checkpoint committed **in
the same SQLite transaction** as the batch it covers. `source.db` ingestion is
one transactional `apply_mutations` — record, facets, edges, FTS, watermark
together; a crash between them cannot exist. Duplicate batches and duplicate
deletes succeed harmlessly. A generation mismatch or sequence gap marks the
source dirty and schedules bounded reconciliation (re-stat against the ledger,
never re-hash, never full rescan).

## Contract 4 — Migration cutover

Per location/source, a durable state machine:

`legacy → building → catching_up → validating → cutover_pending → spine → retired`

Each state defines write routing (legacy store, spine store, or both),
restart behavior (every transition idempotent, crash resumes the state),
projection publication (a source appears in `catalog.db` no earlier than
`validating`), and rollback (possible until `cutover_pending`; after `spine`,
forward-only). The golden parity suite gates `validating → cutover_pending`.
Legacy rows are retained until `retired`. The suite proves a *completed*
migration; this machine is what makes a mid-cutover crash recoverable —
both are required.

## Gate: the bug register

The arena must not feed the drain until the identity-corrupting class is
fixed. Confirmed by direct read (2026-08-18):

- Duplicate-path adds return without updating metadata — modifications never
  land, and rollups inherit the stale sizes (`ephemeral/index.rs:203`).
- Snapshot rewrite is skipped on unchanged *entry count* — renames, metadata
  changes, and balanced delete+create pairs are never persisted
  (`ephemeral/cache.rs:457`, introduced by `f85d2582a`).
- A different drive mounted at a familiar path inherits the old source id via
  root-match fallback (`ephemeral/sources.rs`, register order).
- Attachment is path existence, not identity — a same-named volume routes into
  the absent drive's partition (`ephemeral/cache.rs`, `set_detached` sites;
  the fix is fingerprint-verified attachment on `VolumeMountChanged`, already
  a known follow-up in the transplant plan's slice 1 status).

Reported by the external review, plausible, unverified — verify before or
during the fixes: recursive reindex leaving descendants attached to dead
nodes; registry corruption silently starting empty with save failures
ignored; the snapshot's embedded `source_id` not checked against the expected
source on restore.

## What does not change

One repository; replacement under existing op contracts; hot arena first with
durability as a drain, never a second walk; record identity separate from
content identity; parent-uuid-plus-name as path truth with no closure table
and no stored paths; per-source authority with derived global projections; no
N-way SQLite joins on hot paths; candidate-versus-confirmed content hashes;
locations as policy subtrees; sidecars as content-addressed artifacts;
real-drive use as the acceptance loop. All of it survived a hostile review
unmodified — the collapse is about how many stores coordinate, not about the
model.
