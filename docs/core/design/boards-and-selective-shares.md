# Boards and Selective Shares

> **Status:** Proposed 2026-11-22. Decision record, design only. Nothing here is built, and no build card starts until Jamie approves it.
> **Decides:** the unit a person and an agent use to group media (working name "board", see [Name](#name)), and what a public link to one uploads.
> **Companions:** [source-durability.md](source-durability.md) (the two halves of a store), [tags-and-assertions.md](tags-and-assertions.md) (the assertion shape this reuses), [agent-grants.md](agent-grants.md) and [agent-writes.md](agent-writes.md) (grants and the write rules), [shares-and-remote-libraries.md](shares-and-remote-libraries.md) (the share grant and public links), [../../plans/2026-08-20-architecture-previs.md](../../plans/2026-08-20-architecture-previs.md) (identity and read composition), [../virtual-sidecars.mdx](../virtual-sidecars.mdx) (sidecar layout)

## Context

You want to point at "the Japan trip" or "Tuesday's shoot" and have that mean the
same thing to you, to the app, and to an agent. Today nothing in core holds that.

- A Space (`core/src/domain/space.rs`) is a sidebar layout. Its items are
  pins: `Overview`, `Volume`, `Tag`, `Path { sd_path }`, `Collection { slug }`
  and others. It lives in `library.db` and syncs through the `Syncable`
  framework in `core/src/infra/sync`.
- A tag can mark many files, but a tag is a classification. It has no clips,
  no rule, and nothing that a share or a grant can scope to.
- Search has filters (`SearchFilters` in `core/src/ops/search/input.rs`), but
  nothing stores a search. Main has no saved search.
- A grant (`core/src/ops/grants`) lends one directory of one source. A media
  group crosses directories, sources and devices.
- `files.stream_url` returns a loopback URL from the mounts share. Nothing
  serves a link outside the machine, and nothing uploads.

The footage work adds two things a group must hold. A clip is a frame range of
one file, computed at query time and never stored (the clips contract,
`docs/core/footage-clips.mdx` on branch `footage/clips-and-timeline`, in
review). A shot is a range found by the shot detection job, with an id derived
from the content uuid and its in and out frames, so unchanged boundaries keep
their id. Neither branch is on main yet.

## Decision 1: a board is a new model on the assertion layer

A board is a new model. It is not a Space and not a tag. It reuses the tag
storage shape: definitions copied into every source store that holds a member,
membership rows in the assertion half of the member's source store, and HLC
ordering.

### Why not a Space

- A Space is presentation. `tags-and-assertions.md` already draws this line
  for tags: "`library.db` keeps only what is genuinely local preference: which
  tags are pinned to the sidebar, their ordering, a color override that should
  not travel." A board is a fact about files, so it has to travel with them.
- Space rows sync through `Syncable`, which is the frozen subsystem (see
  [Conflicts](#conflicts-with-the-aug-17-and-aug-20-decisions)). Putting new
  shared rows on it extends what the Aug 20 record says goes away.
- A `SpaceItem` holds a path. A path does not survive a rename, and it cannot
  name a frame range.

A board can still appear in the sidebar. A new `ItemType::Board { board_id }`
pins it, exactly as `ItemType::Tag { tag_id }` pins a tag.

### Why not a tag

A tag would carry almost everything, and the tag tables could take a `kind`
column. The difference is in what each one holds:

| | Tag | Board |
|---|---|---|
| Member | a record (a whole file) | a file, a clip, or a shot |
| Membership by rule | no | yes, optional |
| Hierarchy | yes, by path | no |
| Scope for a grant or a share | no | yes |

Ranges and rules would turn every tag reader into a range-aware reader. Keeping
the two apart costs two tables and keeps tags simple.

### Why the assertion layer

`source-durability.md` names this data directly: the assertion layer holds
"ratings, favorites, corrected titles, tags, album membership a person curated.
Nothing can rebuild it, on any day, for any source, however live the origin is."
([source-durability.md, line 13-15](source-durability.md#L13-L15)). The
architecture previs says the same about reads: "An assertion carries claims —
capture time, display name, an album membership — keyed by path."
([architecture-previs, line 211-212](../../plans/2026-08-20-architecture-previs.md#L211-L212)).

So board membership is assertion data. Reindex and evict never touch it, and
no code path may assume it can be rebuilt.

### What a board holds

A definition and its members.

```sql
-- In every source store that holds a member, and in library.db while it holds none.
CREATE TABLE board_definition (
    uuid          BLOB PRIMARY KEY,  -- v7, stable through rename
    name          TEXT NOT NULL,
    rule          TEXT,              -- versioned search spec, NULL for a hand-made board
    updated_hlc   TEXT NOT NULL,
    origin_device BLOB NOT NULL
);

-- In the assertion half of the source store that owns the member's record.
CREATE TABLE board_member (
    board_uuid   BLOB NOT NULL,
    record_uuid  BLOB NOT NULL,
    external_id  TEXT,               -- rebind evidence
    content_uuid BLOB,               -- rebind evidence, set when known
    in_frame     INTEGER,            -- NULL for a whole file
    out_frame    INTEGER,            -- exclusive, as in the clips contract
    rate_num     INTEGER,
    rate_den     INTEGER,
    shot_id      TEXT,               -- set when the range was a shot
    asserted     INTEGER NOT NULL,   -- 1 add, 0 remove
    hlc          TEXT NOT NULL,
    device_uuid  BLOB NOT NULL,
    PRIMARY KEY (board_uuid, record_uuid, in_frame, out_frame, hlc, device_uuid)
);
```

Three kinds of member, one row shape:

- **File.** `in_frame` and `out_frame` are NULL. Any kind of file can be a
  member: video, photo, audio, a document.
- **Clip.** A frame range at the source's own rate. The fields match the clips
  contract, so a clip returned by `search_clips` can be added to a board as-is.
- **Shot.** A clip whose range came from the shots sidecar. The row keeps the
  frames and the `shot_id`. The frames are the truth. If the shot job reruns
  and that shot id no longer exists, the member keeps its frames and the board
  marks it "shot changed". A member never disappears because a detector moved a
  boundary.

The keys follow `tag_assertion`: `record_uuid` is the key, and
`(external_id, content_uuid)` is rebind evidence, which is decision 5 of
`source-durability.md`. The content uuid is the candidate id, the same one
the media sidecars are keyed by, so a member and its proxy find each other.
Removal is an appended row with `asserted = 0`, never a delete, for the
reason `tags-and-assertions.md` gives: a drive that was in a drawer must not
resurrect a member you removed while it was away.

Folders are not members. "Everything under ~/Footage/Japan" is a rule with a
path scope, so it stays current as files arrive.

Boards are unordered. A sort is a view. An ordered sequence of clips is a
timeline, which `export_timeline` in the clips contract already owns.

### Membership by rule

Yes, a board can be a saved search. A board's members are:

```
rule hits  +  added members  -  removed members
```

- The rule is a versioned `SearchFilters` spec plus a scope (a path, a source,
  or the library). It runs through the one search router at read time, so it
  never writes membership rows of its own. This keeps the previs rule that
  layers compose at read time.
- An added member is a `board_member` row with `asserted = 1`. A removed member
  is a row with `asserted = 0`, which also hides a rule hit. So "the Japan
  folder, minus the blurry ones, plus three clips from the drone card" is one
  board.
- `boards.pin` copies the current rule hits into added members and clears the
  rule. A person uses it to freeze a board, and every share does the same thing
  internally (see [What a share uploads](#what-a-share-uploads)).
- In v1 a rule selects files only. A clip-level rule, such as "every shot where
  I talk about the train", needs `search_clips` to be stable first and comes
  later.

### How it syncs

Nothing new. Each half rides a path that already exists for tags.

- **Members** live in the store of the source that owns the record, in its
  assertion half. A member added on a device that does not own the source goes
  through `assertion_outbox` and `sources.assertions.merge` (`core/src/ops/tags/outbox.rs`)
  with a new kind, `board`. The owner merges it, and replicas receive it with
  the store. Two devices that add and remove the same member offline converge
  by HLC.
- **Definitions** are copied into every store that holds a member, reconciled
  by `updated_hlc`, and kept in a `board_staging` table in `library.db` while
  a board has no members, the same way `tag_staging` works. A detached drive
  carries the names of its boards.
- **A rule-only board** has no members and lives only in `board_staging`.
  `board_staging` does not sync between devices in v1, exactly like
  `tag_staging` today. A rule board made on one device appears on another
  once it has an added member somewhere, or once library-plane assertion sync
  exists.

A definition that meets a foreign library is adopted as-is. Boards have no
convergent slug and are never merged by name. My "Japan" and yours are two
boards.

### Agents and grants

Agents point at a board by its uuid or its name. Two scopes follow from that.

**Read through a directory grant.** An agent with an ordinary grant can list
the boards that have members inside its directory, and read those members. It
sees only members inside the grant. A board does not widen a grant.

**A board grant.** A grant whose scope is a board instead of a directory:

```
grant:
  scope   directory { source_id, subtree } | board { board_uuid }
  caps    read | curate
```

- `authorize` checks membership instead of containment: the path's record has
  a current `asserted = 1` row, or is a rule hit. The membership lookup is an
  indexed read on `board_member`. A rule hit costs a search, so the grant
  resolves the rule once when it is created and on `grants.extend`, and holds
  the result as a pinned set. A rule change does not widen a live grant.
- The agent sees a flat virtual root of members, named by file name, with
  collisions numbered the way keep-both numbers them. Paths stay grant-relative,
  so the existing tools need no change.
- A clip member admits reads of the whole file in v1. The clip tools
  (`read_footage`, `search_clips`) return only the member's range. Range-limited
  byte reads are possible later through the proxy, not through the original.
- `read` is the only file capability. A board grant never writes files.
  `agent-writes.md` keeps writes single-owner inside one granted directory,
  and a board spans sources and devices.
- `curate` lets the agent add and remove members. That is an assertion write,
  not a file write, so the stale-refusal rules for files do not apply. It merges
  by HLC like any other assertion.
- No grant can mint a public link.

## Decision 2: a share uploads a frozen manifest, and nothing else

A share of a board is a public link that serves a copy of what the person chose,
from storage that does not depend on the owner being online. It never exposes
the library and never serves from it.

### What a share uploads

Publishing resolves the board once, into a manifest. The manifest is the whole
contract: the uploader receives the manifest and nothing else, no path, no
source, no directory to walk.

1. Resolve membership: rule hits plus added members minus removed members, the
   same way `boards.pin` does. The share records the resolved list. A board
   that changes later does not change the share.
2. Show the person the list and the total upload size before anything leaves the
   machine.
3. For each member, choose renditions:

| Member | Default | With "include originals" |
|---|---|---|
| Video file | `proxies/720p.mp4`, metadata stripped | adds the original file, unchanged |
| Clip or shot | the member's range cut from the 720p proxy, frame-exact, metadata stripped | adds the whole original, with a warning that it holds more than the clip |
| Photo | a web rendition, long side 2048, metadata stripped | adds the original file, unchanged |
| Audio | a compressed rendition | adds the original |
| Any member | one poster image | |

4. Upload each rendition under a random object id that belongs to this share.
   Ids are never content hashes and never shared between shares, so the store
   cannot tell that two shares hold the same file.

Stripping covers container and EXIF metadata, location included. Originals
upload unchanged, and the confirmation says they can carry location. Captions
from the transcript are off by default and are a per-share option, since a
transcript is text the viewer did not see on screen.

A missing proxy or rendition is generated at publish time by the same jobs
that make sidecars. A member whose owning device is offline waits. The share
page shows the members that have arrived and says how many are pending. The
publish job is resumable and checkpoints per object.

The guarantee is testable: the upload log of a publish equals the manifest,
object for object. That test belongs in the acceptance matrix for the feature.

### Where it goes

To a storage target. The publish job writes to a target interface with three
calls: put an object, delete a prefix, and report usage. The share id is the
prefix. Which targets exist is the custody decision in
[Custody](#how-this-fits-the-open-custody-decision). The job and the manifest
do not change with the target.

The local record of a share is small shared state in `library.db`, like a
grant row: id, board uuid, manifest, options, expiry, revocation time, and the
target's id for the share. It does not sync in v1, again like grants.

### How long it lives

- Every share has an expiry. The default and the choices (for example 1 day,
  7 days, 30 days) are Jamie's call. "No expiry" exists only as an explicit
  choice. `shares-and-remote-libraries.md` already argues for this: "a link
  that lives forever is a link nobody remembers making."
- At expiry the link answers "no longer available" and the target deletes the
  share's prefix. The bytes are gone, not hidden.
- Extending a share moves the expiry. It never re-uploads.
- Updating a share is a new publish against the same id. It uploads new
  members and deletes removed ones, and shows the difference before it runs.

### Revoking

- `shares.revoke` sets `revoked_at`, tells the target to stop serving, and
  deletes the prefix. The link answers "no longer available" on the next
  request.
- Media is served through short-lived signed URLs, minutes long, so a page left
  open stops playing soon after revocation. Anything a viewer already
  downloaded stays downloaded. The confirmation says so.
- Revoking needs the device that published the share, or an account page on the
  service when the target is the hosted one. Library-plane sync of share rows
  would let any device revoke. That waits for assertion sync.

### View-only and download

- **View-only** streams renditions and shows no download control. This stops
  casual saving and nothing more: a viewer can still record the screen. The UI
  says "view only" and does not promise protection.
- **Download** adds per-item download and a zip of the share. Download offers
  what was uploaded: renditions, and originals only when the share included
  them.
- An optional password gates the page, and the owner sees when the link was
  opened and how often, as `shares-and-remote-libraries.md` describes for all
  public links.

## How this fits the open custody decision

### What main says

Main states a custody principle, and leaves open whether the hosted service ever
holds bytes:

- "No hosted Spacedrive service becomes the required authority over a library."
  ([product-direction.mdx, line 79-80](../product-direction.mdx#L79-L80))
- "A public link serves from the machine that owns the content, so the link is
  live while that machine is reachable and dead while it is not. This is the
  correct default and a poor surprise. The relay can hold pinned bytes for a
  share that must survive the source going offline, which is a storage service
  rather than a link service"
  ([shares-and-remote-libraries.md, line 84](shares-and-remote-libraries.md#L84))
- The older task epic in the frozen `.tasks/` tree took the opposite position:
  "Bytes never transit sd.app — the browser dials the user's core directly
  through our relay over QUIC."
  ([.tasks/core/SHARE-000-public-shares-epic.md, line 13](../../../.tasks/core/SHARE-000-public-shares-epic.md#L13)).
  `.tasks/` is frozen and not the register, but it is the only public text
  that decides the question, and it decides it against uploading.

So the open question, as the docs frame it: does the hosted service hold
share bytes at all, and if it does, can it read them? A selective share that
works while the owner is offline is the pinned-bytes case. It needs an answer.

### Options

| | Bytes held by | Service can read them | Works with owner offline | Cost to build |
|---|---|---|---|---|
| A. Hosted, readable | the hosted service | yes | yes | lowest |
| B. Hosted, encrypted on the device | the hosted service, as ciphertext | no | yes | the viewer decrypts in the browser, so streaming is more work |
| C. Live from the owner | the owner's device, through the relay | no copy held | no | no upload at all, availability follows the source |
| D. Your own storage | a bucket or domain the person controls | that provider | yes | a target adapter and credentials |

In option B the key travels in the link's fragment (`#k=...`), which a browser
never sends to a server. The old epic reserved exactly that fragment. Holding
the link means holding the key, so revocation still works by deleting the
ciphertext. The service can no longer transcode, scan or thumbnail server-side,
which is why the device produces every rendition before upload.

### Recommendation

Build the manifest and the target interface first. They are the same for every
option. Then:

- Ship D and C first in the order `shares-and-remote-libraries.md` already sets
  (self-hosted public links before the relay).
- For the hosted target, choose B over A. B keeps the custody line of the
  product direction: the hosted service stores ciphertext it cannot read and
  never becomes an authority over the library. A breaks that line for every
  shared file.

This is a recommendation, and the custody decision stays open until Jamie makes
it. Pricing, retention commitments and the hosted service's own design are out
of scope for this public document.

## Conflicts with the Aug 17 and Aug 20 decisions

The decisions on main that these refer to are the sync freeze and the teardown
built on it. The exact lines:

- Aug 17: "the sync freeze retired the premise"
  ([2026-08-17-record-table-transplant.md, line 3](../../plans/2026-08-17-record-table-transplant.md#L3)),
  and "Sync (frozen by decision)."
  ([line 230](../../plans/2026-08-17-record-table-transplant.md#L230))
- Aug 20: "Sync is frozen by decision, and nothing reads `sync_enabled`."
  ([2026-08-20-entries-teardown.md, line 28](../../plans/2026-08-20-entries-teardown.md#L28)),
  "**Sync** — already frozen; its tables and service go with the schema."
  ([line 51](../../plans/2026-08-20-entries-teardown.md#L51)), and the previs
  table row "sync tables, `service/sync`, `infra/sync` | frozen; HLC retained
  for assertion ordering"
  ([2026-08-20-architecture-previs.md, line 256](../../plans/2026-08-20-architecture-previs.md#L256)).

The 2026-11-21 amendment allows single-owner writes. On main it is recorded in
`agent-writes.md`: "Writes are single-owner and write-through." A stale write is
"refused as stale", a create collision means "both are kept", and "Spacedrive
never combines two versions of a file."
([agent-writes.md, line 11-27](agent-writes.md#L11-L27))

What this record does against them:

1. **Board membership merges.** Members from several devices merge by HLC.
   That is assertion merge, which the Aug 20 previs kept HLC for and which tags
   already do. The amendment governs file writes, and its "merges of any kind"
   are out of scope for writes only
   ([agent-writes.md, line 25-26](agent-writes.md#L25-L26)). No conflict, but
   the plan docs should say so, since "frozen sync" and "no merges of any kind"
   both read as covering assertions.
2. **Definitions avoid the frozen framework.** Spaces still sync through
   `Syncable` on main, although the Aug 20 teardown says that framework goes.
   This record keeps boards off it and on the tag path instead. The cost is that
   a rule-only board does not reach another device in v1.
3. **Rules are matched at query time.** The previs says "Collections are
   classified at index time, never matched at query time."
   ([line 226](../../plans/2026-08-20-architecture-previs.md#L226)). A board rule
   is matched at query time. These are different things: a collection is a
   built-in classification flag, a rule is a person's saved query. The name
   "collection" stays with the built-in ones.
4. **Shares upload.** `shares-and-remote-libraries.md` (Aug 19) says a share
   "reads from one authority" and moves no bytes, and puts the relay last.
   Selective shares hold pinned bytes. That is the storage service the same doc
   names, so this record extends that doc and does not contradict it. Its share
   record needs a `board` scope beside `subtree`.
5. **No file writes.** Neither a board nor a share writes to a file. Nothing
   here touches the single-owner write rules.

## Name

The type, op prefix and UI all use one word. Two natural words are taken:
"collection" names the built-in index-time collections (`ItemType::Collection`),
and "album" is a model of the Photos extension (`extensions/photos`), which a
board will sit under.

| Name | Reads as | Collides with |
|---|---|---|
| **Board** | "the Japan board", "add these shots to the board" | nothing in core |
| Reel | footage, natural for shoots | the reel name field in EDL export, and short-video "reels" |
| Set | neutral, an old word for a photo group | `Set` in code, "settings" and the verb in the UI |
| Bin | the editor word for a group of clips | reads as trash outside editing |

**Recommendation: Board.** It works for a trip, a shoot, and a mix of photos
and clips, and it collides with nothing. Its cost is a hint of a spatial
canvas, which v1 does not have.

## Open questions for Jamie

1. Pick the name.
2. Board grants: approve the `board` scope and the `curate` capability, or keep
   grants directory-only for now.
3. Share expiry: the default, and whether "no expiry" exists.
4. The custody decision: A, B, C or D for the hosted target.
5. Captions in shares: off by default, as proposed?
6. A clip member's original: upload the whole file with a warning, as proposed,
   or offer no original for clips.
