# Tags and Assertions

> **Status:** Executed 2026-09-17 by `docs/plans/2026-09-17-tags-on-source-stores.md`,
> with three reconciliations noted inline: assertions carry `content_uuid BLOB`
> following `record_overlay` rather than `content_id TEXT`; definition merge is
> row-level last-writer-wins on the single `updated_hlc` the schema defines
> rather than per field; and `record_uuid` is NOT NULL, leaving content-keyed-only
> applications an open question. The slug folds case and composes to NFC.
> **Captured:** 2026-08-21, from James's direction that tags are a source primitive
> **Companions:** `docs/plans/2026-08-20-architecture-previs.md` (the identity model this extends), `docs/plans/2026-08-18-storage-consolidation.md` (contracts 1–2, artifact classes), `docs/plans/2026-08-20-entries-teardown.md` (phase 2, where tags re-anchor), `docs/core/design/file-backed-sources.md` (late binding of assertions)

## The rule

**A source carries everything needed to read its own tags.** Definitions and
applications both live in `source.db`, so a drive that arrives at a library that
has never seen it presents named, colored, hierarchical tags rather than opaque
uuids. `library.db` holds no tag definitions. `catalog.db` projects the union.

This is what makes detaching a source a real operation instead of a partial one.
Unplug the drive and the knowledge goes with it, because the knowledge was never
anywhere else.

## Definitions and applications

Two different things wear the word "tag" and they have different lifetimes.

A **definition** is the tag itself: a name, a place in a hierarchy, a color. It
is small, mutable, and shared by every record it touches.

An **application** is the claim that a definition applies to something. It is
tiny, append-only, and belongs to exactly one record or one piece of content.

Contract 2 already places applications in `source.db` — the identity ledger
exists precisely so tags keyed on record uuids survive an index rebuild. The
half that was still wrong is definitions. With definitions in `library.db` and
applications in the source, a detached drive travels carrying rows that say
`tag_uuid = 7f3a… applies to record X` and nothing that says what `7f3a…` is.
The tag travelled as a foreign key into a database that stayed home.

## A tag is an entity with a convergent rebind key

The previs commits to three identity strategies and is emphatic that conflating
them is the classic failure. Content is a value, so its id is derived from its
bytes. A record is an entity, so its id is assigned and rebound by evidence. A
mutation is an event, so it is ordered by HLC. A tag is none of these and the
architecture never said which it is.

It is an entity with a convergent rebind key, which is the record strategy with
different evidence:

| | assigned identity | rebind evidence |
|---|---|---|
| record | uuid v7 | relative path, inode, size+mtime — two must agree |
| tag | uuid v7 | `slug_id = v5(TAG_NS, normalized_path)` |

Assigned, so renaming a tag does not shatter every application that references
it. Convergent slug, so a drive arriving at a foreign library can find that
library's existing "Work" instead of silently creating a second one.

The slug derives from the full normalized path, not the leaf name. `Work/Clients/Acme`
and `Personal/Acme` are different concepts and must not merge on the strength of
a shared last segment. A parent rename therefore changes a child's slug, which is
harmless: the slug is a merge key consulted only when a definition arrives from
elsewhere, never an identity and never a foreign key.

## Definitions replicate into every source that uses them

A definition is copied into each `source.db` that holds an application of it.
Not moved, copied. A few hundred bytes times a few hundred tags is nothing
against an index, and the payoff is that every source is self-describing.

`library.db` keeps only what is genuinely local preference: which tags are pinned
to the sidebar, their ordering, a color override that should not travel. Those
are facts about this machine's UI, not about the tag.

The replica is reconciled by HLC on the mutable fields. Rename a tag while a
drive is detached, reattach it, and the definition with the later HLC wins on
both sides. This is the one place the retained HLC does work that nothing else
can do.

```sql
CREATE TABLE tag_definition (
    uuid        TEXT PRIMARY KEY,  -- v7, stable through rename
    slug_id     TEXT NOT NULL,     -- v5(TAG_NS, normalized path), merge key only
    path        TEXT NOT NULL,     -- 'Work/Clients/Acme', full ancestor chain
    color       TEXT,
    icon        TEXT,
    updated_hlc TEXT NOT NULL,
    origin_device TEXT NOT NULL
);
```

**The cost, stated plainly:** a tag created and applied to nothing has no source
to live in. `library.db` holds a small unbound-definitions table for that case,
and a source adopts the definition the first time it is applied there. This is
the one denormalization the model does not remove. The alternative is that
creating a tag is silently a no-op until you use it, which is worse.

## Applications carry two keys

An application names a record uuid always, and a content id once hashing has
reached the file.

```sql
CREATE TABLE tag_assertion (
    tag_uuid    TEXT NOT NULL,
    record_uuid TEXT,
    content_id  TEXT,              -- set when the hash lands
    asserted    INTEGER NOT NULL,  -- 1 apply, 0 remove
    hlc         TEXT NOT NULL,
    device_uuid TEXT NOT NULL
);
```

The record key answers "what is tagged at this path". The content key answers
"everywhere these bytes are". Neither substitutes for the other, the same rule
the previs states for record and content ids generally.

Today's schema has both columns and forbids both being set: `user_metadata`
carries `entry_uuid` and `content_identity_uuid` with "exactly one of these is
set" and a priority ladder deciding which wins. That exclusivity is why
content-attached tags degrade. A content-scoped row has no record to render
against while browsing a directory, and a record-scoped row cannot answer the
cross-drive question. Carrying both on one row removes the ladder and the
choice.

The content key is what dissolves the reconciliation problem the previs marked
open. Tag a photo on drive A and tag the same photo independently on drive B,
and both sources write `(tag_uuid, content_id=X)`. Content ids are convergent, so
those are not two assertions that later need merging. They are the same
assertion, computed twice, offline, with no coordination. They collapse at
projection time by taking the latest HLC.

Hashing does not happen at first sight, so an application made during the walk is
record-keyed and acquires its content key later. That late binding is already
required by the drain pipeline and is exactly the mechanism `file-backed-sources`
uses when adapter assertions arrive before the records they describe.

## Scalar assertions take the same shape

A tag application is one kind of assertion. A rating, a favorite, a corrected
title are another: scalar claims about one record, where the last writer wins
outright and there is nothing to enumerate across sources. Those stay in
`record_overlay` rather than folding into `tag_assertion`, because a JSON blob
of scalars cannot answer "which photos have this tag" and a tag table has no
use for a column per scalar somebody invents.

Two tables, one row shape. Both key on the record uuid, both carry
`(external_id, content_id)` as rebind evidence, and both carry `hlc` and
`device_uuid`. That is what makes "the assertion layer" a thing the store can
name: the half of a `source.db` that no ingest can rebuild, that reindex leaves
alone, and that sync merges rather than replicates.
`docs/core/design/source-durability.md` carries the split and what it buys.

`record_overlay` today has neither the key nor the columns. It is
`(type, external_id)` with an `updated_at` wall clock and no device, which is
the shape the previs's decision 2 rules against and a merge cannot use.
`2026-08-22-source-convergence.md` P1.6 is the fix.

## Removal is an assertion, never a delete

Unapplying a tag appends a row with `asserted = 0`. The state of a tag on a
record is the latest assertion by HLC.

This is load-bearing for detached drives specifically. If removal were a row
delete, a drive that spent a month in a drawer would return holding a stale apply
row for a tag you removed while it was gone, and the tag would resurrect. Ordered
append-only makes the removal win because it happened later, which is the only
thing that can be known without asking the drive.

## Hierarchy travels as a path

A tag hierarchy is a graph, and a graph does not partition by source. A drive
carrying `Acme` whose parent `Clients` lives in the host library arrives with a
dangling edge.

A definition therefore stores its full ancestor path rather than a parent
pointer, and grafts by path on arrival. Missing ancestors are created as bare
definitions. The closure table becomes a rebuildable projection over the paths,
which is the same call the previs already made when it deleted `entry_closure`.

The alternative, if denormalized paths prove wrong, is that source-carried tags
are flat and hierarchy stays library-only. That is simpler and loses structure on
travel. It is worth knowing it exists, but a tag tree that does not survive
moving a drive is not really the feature.

## Reconciliation, enumerated

Every case, so the shape of the work is visible:

| Case | Resolution |
|---|---|
| Drive leaves and returns to the same library | Nothing. Uuid stable, definition on both sides, applications came home. |
| Foreign library, slug unknown there | Adopt the definition. The library gained a tag. |
| Foreign library, slug known, different uuid | The genuine merge. **Offered to the user, never automatic.** |
| Same uuid, definitions diverged | Latest HLC wins per field. |
| Same content tagged on two sources | Not a conflict. Identical rows, collapsed at projection. |

The first row is the overwhelming majority of real use and it costs nothing. The
third is the only case with a decision in it, and it must stay a decision: my
`Work` and a stranger's `Work` may not be the same concept, and a silent union is
unrecoverable.

## Projection and trust

`catalog.db` sweeps source stores into the global tag view: which tags exist
anywhere, what they are applied to, across attached and detached sources alike. A
detached source's tag rows in the catalog are **availability-bearing replicas**
under the consolidation doc's artifact classes, so they are retained rather than
evicted when the previous-good generation is all there is.

The trust rule applies unchanged. A catalog row is enough to browse by tag or
render a badge. It is not enough to make a destructive decision, and a missing
row is never proof a tag was removed.

## What the definition may hold

Portability is a budget. Every field on a definition is a field that has to
survive travel and, when two definitions meet, merge. `composition_rules` as
opaque JSON cannot merge, so it cannot be on a definition.

The current schema (`m20250115_000001_semantic_tags`) carries canonical, display,
formal and abbreviated names, aliases, namespace, tag_type, privacy_level,
search_weight, is_organizational_anchor, plus JSON `attributes` and
`composition_rules`; applications carry applied_context, applied_variant,
confidence and instance_attributes. Behind it sit nine registered ops
(`ops/tags`, 4,824 lines) over `ops/metadata/manager.rs`, including a
`TagConflictResolver` and a `TagClosureService`.

The teardown's phase 2 currently reads as a re-key, and a re-key would drag all
of that across. It is a rewrite against the small definition above. Anything not
in that table is deleted rather than ported, and returns later as a facet or an
assertion if it earns its way back.

## Consequences for the plan

> Executed. The phases below described the entries-teardown sequencing this
> design originally targeted; `2026-09-17-tags-on-source-stores.md` carried
> the build and its execution record. The paragraphs stay as rationale.

**Phase 1** owns this. The durable substrate is where these two tables get their
shape, and `record_overlay` is re-keyed to match `tag_assertion` on the way in
rather than folding into it (see "Scalar assertions take the same shape"). Its
key was `(source_id, type, external_id)` in the two-file layout and is
`(type, external_id)` now that the durable layer moved into `source.db`, and in
both the external id is a path, which is the failure previs decision 2 exists to
prevent. Settling this now is a paragraph. Settling it after phase 1 is a
migration.

**Phase 2** re-anchors tags as a rewrite, not a re-key, and no longer keeps
definitions in `library.db`.

**Phase 5** gains the tag projection alongside placement rows.

## Open

- **Whether an application should ever be content-keyed only.** Today every
  application starts record-keyed. A tag applied to content the library knows
  about but has no local record for has nowhere to live. The catalog is the
  obvious home and is the wrong one, because it is authoritative about nothing.
- **Tag deletion across detached sources.** Deleting a definition while a drive
  is away leaves applications on that drive pointing at a tag the library no
  longer has. It reads as the foreign-library adoption case on return, which
  resurrects the tag. Whether that is correct or needs a definition tombstone is
  unresolved.
- **Whether spaces follow the same model.** Space definitions sit beside tag
  definitions in `library.db` today and the arguments look similar, but a space
  spans sources by construction, so the ownership question has a different shape.
