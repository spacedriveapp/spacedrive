# Extension Kinds: adapters, enrichers, apps

> Where the lines are drawn between something that brings data in, something
> that works out what the data is, and something a person opens. The tree has
> the first and third and has been treating the second as a special case of the
> first, which is why Apple Photos is a strange adapter and transcription has
> nowhere to live.
>
> **Related.** `2026-08-25-extensibility.md` owns presets, the dependency
> registry and the plugin wrapper; this document says what the wrapper wraps.
> `docs/core/design/extension-points.md` inventories the seams; this document
> sorts them. `2026-08-22-source-convergence.md` decision 5 and
> `docs/core/design/file-backed-sources.md` supply the two rules everything
> here follows from. `docs/core/design/source-durability.md` supplies the
> third.

## The three rules already in force

1. **One store shape.** `record`, `facet_<model>`, `content`, `edge`,
   `record_overlay`. Cross-source search and the catalog join on it, so
   nothing that plugs in may change it. Schema is pluggable at the facet, and
   only there. (Convergence decision 3; `crates/store/src/lib.rs`.)
2. **The index owns the record, the contributor adds knowledge about it.**
   Stated for Apple Photos in `file-backed-sources.md`. It is a general rule
   and this document treats it as one.
3. **Generation is rebuildable while the origin answers; assertion never is.**
   `source-durability.md`. It decides which table a fact belongs in, and it
   turns out to decide which kind of extension produced it.

## Three kinds, sorted by input

| | takes | produces | owns records | rebuildable |
|---|---|---|---|---|
| **Adapter** | an origin: a drive, a service, a catalog file | records and facets into a source store | foreign adapters yes; file-backed adapters no | while the origin answers |
| **Enricher** | records that already exist | facets, edges, and records only for things that are not files | no | always, it is derivation |
| **App** | a person | record types of its own, views, questions | its own nouns only | never, it is all assertion |

**What has an origin is an adapter. What has an input record is an enricher.
What has a user is an app.**

All three are payload types of the plugin manifest that
`2026-08-25-extensibility.md` says to build last. All three program against
`crates/sdk`. The manifest differs only in which of `ingest`, `jobs`, `models`
and `views` it declares, and a single plugin may declare more than one: a
Lightroom plugin is a file-backed adapter and a set of views.

### Adapters

Unchanged from today in mechanics, narrowed in scope. The eleven under
`adapters/` are foreign adapters: their records are primary because the
subjects are not files on any local disk. `adapter.toml` declares
`[models.*]`, `DataTypeSchema` turns that into `facet_<model>` tables, and
`sync.py` writes records. The filesystem walker is the first-party adapter
for drives, and `filesystem_schema()` is its `adapter.toml`.

A file-backed adapter (Apple Photos, Lightroom, Capture One) has an origin,
so it is still an adapter, but rule 2 means it does not mint records for the
bytes a drive already indexes. Its file rows become facets on the filesystem
record; its albums and people are records with edges to filesystem uuids.
Once that is true it behaves exactly like an enricher whose input happens to
be a sqlite file, and the taxonomy below treats it as one. The remaining
difference is that an adapter has credentials and a schedule.

### Enrichers

The kind that has been missing. Transcription, OCR, EXIF and video metadata,
content identity, face and voice clustering, scene detection, entity
extraction, embeddings: none of them has an origin. Each reads records from a
store and writes back. Their outputs are exactly three things:

- **A facet.** `facet_image`, `facet_video`, `facet_transcript`. One row per
  record, typed columns, declared through the same `ModelDef` an adapter
  uses. P2.5 (3) of the convergence plan already asks for the first two.
- **An edge.** `appears_in`, `mentions`, `same_scene_as`. Between records in
  the same store.
- **A record, only for a thing that is not a file.** A person, a place, a
  segment. Rule 2 forbids anything else.

An enricher is a set of job kinds plus the models those kinds write. That
puts it under the line `2026-08-25-extensibility.md` draws: **a model
composes registered kinds, a plugin adds kinds.** An agent may run
"transcribe these forty and diarise the ones with two voices" because
transcribe and diarise are registered; it may never define transcribe.

### Apps

Photos, the Analyzer, Story Studio. An app declares record types nothing
else writes to, ships views over core types and its own, and asks a person
questions. It does not ingest. `extension-points.md` already says an app
ships a source; the section on where an app's records live carries that
through.

## Where the schema is pluggable

The spine is fixed and the facets are the plug. `record.type` is an open
string, `edge.type` is an open string, and every kind of extension declares
`ModelDef`s to get its columns. That is what "plugging in a new data schema"
means here, and it is enough for a DAM, an email archive, or a story project.
It stops short of letting a plugin add a column to `record`, because the
catalog reads `record` across every source and a second shape means two of
everything downstream.

### Shared nouns are core schema

`person`, `place`, `event`, `segment` will be written by a face enricher, a
transcript enricher, a calendar adapter, Photos and Story Studio. If each
declares its own `person`, decision 5's "definitions replicated into every
source that uses them" fails, because they are no longer one definition.

So: **a noun more than one extension writes to is declared by core. A noun
only one extension writes to is declared by that extension.** Core owns the
four above and their facets. An app owns `story_project` and `chapter`
because nothing else cares.

`segment` is the one that is new. An event's evidence is a span inside a
file: 1:12:04 to 1:14:30 of a recording, page three of a PDF, one frame. The
record table addresses files. A `segment` record carries the parent file's
uuid and a typed span, gets its own transcript facet and its own thumbnail,
and is what edges point at when they mean part of a file. It is a record
rather than an edge attribute because segments are reused across events and
need their own derived data.

## Which table a fact goes in

Rule 3 answers this without a new column, and it is worth stating because the
Story Studio vision lists ten epistemic states and most of them are
positional.

| the fact | who produced it | where it lives | rebuildable |
|---|---|---|---|
| faces 41, 87 and 203 cluster together | enricher | `facet_face_cluster` | yes |
| that cluster is Muudea | person | `record_overlay` | no |
| this recording was made on 2020-02-01 | adapter, from metadata | `facet_video` | yes |
| this recording was made in the North Vancouver house | person | `record_overlay` | no |
| this email supports that claim | person or agent, confirmed | `edge`, with provenance | no |

Machine inference lives in facets and can be regenerated. A person's
confirmation lives in the overlay and cannot. "Machine-inferred" against
"user recollection" is which table the row is in.

What still needs an attribute is the relation between a claim and its
evidence: supported, corroborated, disputed, contradicted, dramatised. Those
are properties of the edge, and `edge` today carries `(src, dst, type, ord,
created_at)`. It needs `provenance TEXT` and `recorded_at INTEGER`, the
second because "what a participant recalled, and when" is a fact about the
recollection. This is a schema addition to make in P2.5 while facets are
being declared, since edges written before it will not carry it.

## Where an app's records live

An event references a Gmail record in one store and a video on a drive in
another. `edge` relates records within one source. Neither store is the right
home and the catalog is a derived index, so it is not either.

**An app's records live in a source the app owns.** Origin is the person.
The generation half is empty; everything in it is assertion. Its edges hold
foreign uuids, and the catalog resolves them the way it resolves every other
cross-source reference. `extension-points.md` reached the same place from the
isolation direction: a misbehaving app is one folder.

What this buys:

- No new machinery. A project is a row in `sources` and a directory under
  `sources/<id>/`.
- Portability. Copy the directory and the edges come with it.
- Deletion is deletion of a source, with the same confirmation.
- Durability follows rule 3 automatically, because the store is all overlay.

What it costs: `sources.data_type` gains a value for this, `adapter_id` is
null, `root` and `volume_uuid` are null, and anything that assumes a source
has an ingest has to tolerate one that does not. That is small and mostly
already true for adapter sources between syncs.

## Identity across stores

A person record is replicated into every source that has an edge to it, with
one uuid, per decision 5. An enricher that clusters faces across three drives
writes the same `person` definition into all three stores. The catalog joins
them; the stores never reference each other directly. This is the tag rule
applied to people and it holds for places and events the same way.

## What each kind may do

Capability grants name the seam (`extension-points.md`, Permissions). Sorted
by kind:

| | may read | may write | runs |
|---|---|---|---|
| adapter | its origin | records, facets, edges in its own store | out of process, on a schedule |
| enricher | records in stores it is granted | facets, edges, core-noun records in those stores | as jobs, in or out of process by kind |
| app | stores it is granted, through the SDK | its own store; overlay in others when a person acts | as a client |

An enricher never writes another kind's facet. An app never writes a facet at
all; when a person corrects something through an app, that is an overlay
write. The one in-process exception is the pipeline tier from
`extension-points.md`: facet extractors that run inside indexing because a
per-file round trip would destroy throughput. Those are enrichers by this
taxonomy and Tier 3 by that one, and both documents agree they land last.

## What this changes in the tree

Little, and most of it is naming.

- `file-backed-sources.md` becomes the enricher rule, with Apple Photos as
  its first example rather than its only one.
- `SidecarKind` (`core/src/ops/sidecar/types.rs`) is a closed enum of seven
  values. An enricher that derives something new needs a kind to store it
  under, so the enum opens, which `extension-points.md` already lists as a
  prerequisite.
- `filesystem_schema()` gains `facet_image` and `facet_video` (P2.5 (3)),
  and core gains model declarations for `person`, `place`, `event`,
  `segment`.
- `edge` gains `provenance` and `recorded_at`.
- `sources` tolerates a row with no ingest.
- The plugin manifest, when it arrives, carries `ingest`, `jobs`, `models`
  and `views` as separate sections rather than being an `adapter.toml` with
  extras bolted on.

## Phases

1. **Declare the shared nouns.** `person`, `place`, `event`, `segment` as
   core `ModelDef`s, beside `filesystem_schema()`. Add `provenance` and
   `recorded_at` to `edge`. This goes in with P2.5 so no facet or edge is
   written against the old shape.
2. **Name the enricher.** Rewrite `file-backed-sources.md` as the enricher
   rule. Register the existing derivations that are already job kinds
   (content identity, thumbnails, EXIF, video metadata) as enrichers writing
   facets. Open `SidecarKind`.
3. **Apple Photos becomes an enricher.** The rewrite `file-backed-sources.md`
   already specifies, done under the name it now has.
4. **An app-owned source.** `sources` accepts a row with no ingest; the SDK
   exposes create, open and write for it. Photos is the dogfood, because its
   albums are the first records nothing else owns.
5. **The first new enrichers.** Transcription with word timecodes writing
   `facet_transcript` on files and segments; OCR; face clustering writing
   `person` records and `appears_in` edges. Each is a job kind with a preset
   and a native dependency, which is what `2026-08-25-extensibility.md`
   builds toward.
6. **The manifest.** With three payload types settled, the wrapper has a
   shape to wrap.

Phases 1 and 2 sit inside the convergence plan's P2.5 and cost a schema
declaration and a document. Phases 3 through 6 wait for the teardown, for the
reason the extensibility plan gives: a manifest over a moving target gets
written twice.

## Open

- Whether `segment` spans are one typed column (`span TEXT` holding a
  time range, a page, or a rect by kind) or per-kind facets. One column is
  simpler and the kinds are few; per-kind facets are queryable. Leaning one
  column until a query needs otherwise.
- Whether an enricher may write a facet on a record in an app-owned source.
  Transcribing an event's narration recording says yes; the table above says
  an app's store is all overlay. The likely answer is that a derived facet
  on an assertion record is still generation, and the "all overlay" claim is
  about what the app itself writes.
- What a person-origin source's `TrustTier` is. `Authored` is the honest
  value and also the most permissive one for screening, which is the right
  outcome for a person's own project and worth saying deliberately.
- Whether Photos and Story Studio are apps or first-party views over core
  nouns. The answer above is that they are apps that own few nouns, and the
  boundary is who writes the nouns nobody else does. If that leaves an app
  owning nothing, it is a view, and that is fine too.
