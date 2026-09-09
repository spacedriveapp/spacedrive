# Archive Requirements, from the reference corpus

> A private personal archive is being assembled by hand, with scripts, as the
> prototype for what Spacedrive should do natively. This is the register of
> what that work has asked for, each item mapped to the primitive that answers
> it and the phase it lands in. Nothing here is a new plan; it is the
> requirements the existing plans have to satisfy, kept in one place so they
> are not lost between voice dumps.
>
> **Related.** `2026-08-30-extension-kinds.md` names the three kinds of
> extension and the core nouns. `2026-08-27-storage-map.md` is the whole-drive
> walk. `2026-08-25-filesystem-intelligence.md` is the agent loop.
> `2026-08-22-source-convergence.md` is the register the schema work lands in.

## What the corpus is

A directory of immutable evidence collections with sha256 manifests, a
knowledge layer of Markdown notes with typed frontmatter (event, person,
place, project, source, artifact, claim), narratives on top, and a `scripts/`
directory that does the ingest and enrichment. It opens as an Obsidian vault
and is versioned with git and git-lfs.

Twelve scripts, each of which is a Spacedrive job in disguise:

| script | what it is in Spacedrive |
|---|---|
| `manifest.py` | content identity (`content.integrity_hash`) |
| `check.py` | referential integrity the store gets from `edge` |
| `yt_catalog.py` | a web adapter listing a channel |
| `yt_ingest.py` | a web adapter fetching a video into a bundle |
| `yt_beats.py` | enrichers: scene detection (ffmpeg), video understanding (Twelve Labs) |
| `photos_index.py` | the Photos file-backed adapter |
| `photos_faces.py` | the face enricher, writing `person` |
| `photos_timeline.py` | temporal clustering of photos into runs |
| `twitter_export.py` | a foreign adapter over a local platform export |
| `discord_dm_export.py` | a foreign adapter over an API |
| `archive_url.py` | a web page adapter |

The `.eml` files are the Gmail adapter's file-backed twin: the same records,
read from disk instead of an API.

## The bundle

`yt_ingest.py` writes one directory per video:

```
<date> <title> [<id>]/
  video.mp4
  info.json                  channel, upload date, counts, description, comments
  thumbnail.jpg
  transcript.youtube.en.txt  and .en-orig
  video.en.json3             timed captions
  scenes/scene_NNNN.jpg      ffmpeg scene cuts
  beats.md                   chapters and beat descriptions from a video model
  twelvelabs.json            the raw model output
  MANIFEST.md
```

This is a rich file type built by hand: one primary file with everything
known about it beside it, in a shape any tool can read. The requirement is
that Spacedrive presents this as **one record of type `web_video`**, tied to
`video.mp4`, whose Inspector shows the thumbnail, transcript, chapters, view
count and comments, and whose scene stills are `segment` records.

Two ways a bundle arrives and both have to work:

- **A directory convention**, like this one or `.photoslibrary`. The walker
  already treats bundles as one thing for browsing (rules as lenses). The
  enricher reads the members and writes facets on the primary.
- **Sibling files**, the yt-dlp default: `x.mp4`, `x.info.json`, `x.en.vtt`,
  `x.webp` in one flat directory. Same enricher, different discovery.

Either way the derived material stays as files beside the original, which is
the archival stance the corpus takes, and the store holds the structured
reading of them. If the store is lost the files still say what they are.

## The register

Every idea from the dumps, the primitive that answers it, and where it lands.

### Data

| idea | primitive | lands |
|---|---|---|
| A YouTube video that shows comments, transcript, visual summary, thumbnail, view count, tied to a real file | `web_video` facet written by a bundle/sibling enricher; `segment` for scenes; `facet_transcript` | enricher phase, first one |
| Twitter archive, Discord DMs, `.eml` on disk as sources | foreign adapters over local exports; the Gmail adapter's schema reused for `.eml` | adapters, after P3 one registry |
| Photos library data pulled out (faces, places, timeline) | Photos as a file-backed adapter plus face and place enrichers, per `file-backed-sources.md` | enricher phase |
| Batches of photos tied to events, not just generic moments | `event` records with `part_of` edges from files; temporal clustering proposes them, a person confirms | enricher phase plus agent loop |
| Understanding what is in photos | image description enricher writing a facet; face enricher writing `person` | enricher phase |
| Transcribe CCTV, camera footage, stream recordings | transcription enricher with word timecodes; scene detection; `segment` | enricher phase, needs native deps |
| AI watches a video and gives beats | video understanding enricher over a user-selected model; output is `segment` records with descriptions, machine-derived, in a facet | enricher phase |
| Lost dates in copied files | temporal constraint propagation from `part_of`, `before`, sibling and container evidence; inferred range in a facet with its derivation | after the event facet exists |
| Previsualise a directory: what is important | the filesystem intelligence agent loop over a mapped drive | P5 catalog plus agent loop |

### Structure

| idea | primitive | lands |
|---|---|---|
| A project that owns notes, research, downloads, events, and references files elsewhere | an app-owned source (origin: the person), referencing filesystem sources by uuid through the catalog | after P4 |
| Files move around; the average person has no git | identity by content (`content.uuid`) and by volume-relative path (`sources.volume_uuid` + `root`); assertions rejoin files when they reappear | P2.5 (content identity), P3 |
| Intelligence travels with the files | per-source `data.db` and sidecar tree on the medium, a per-source setting | P2.5 (4), decided |
| Never a library that gets lost leaving half the data | the store is per source, the library holds only device claims and the registry; assertions are content-keyed so they survive separation from the store | decided; source-durability |
| A hidden folder gets left behind on a naive copy | see "The hidden folder" below | open |
| Version control, because agents will write here | an append-only assertion log, not git; see below | P2.5 reserves it, sync needs it |
| Technical people want it all in one container they can time-travel | the Markdown vault as the export form of an app-owned source, round-tripping with the store | after app-owned sources |

### Interface

| idea | primitive | lands |
|---|---|---|
| A grid of events presented like files | the Explorer renders any record type; per-type card renderer | renderer registry, `extension-points.md` |
| A grid of people as cards, click in for more | same, `person` cards; the Photos app needs this first | with Photos |
| Space bar previews an event | Quick Preview dispatches on record type, not only content kind | renderer registry |
| The Inspector shows facets | one panel per facet; the `web_video` panel is the first | with the first enricher |
| A timeline view | a fourth Explorer view over anything with dates, using inferred ranges and showing precision | after the event facet |
| A dedicated Story Studio surface shaped like the vault layout | an app on the SDK, after the generic surfaces prove what it needs | last |

### Generative

| idea | primitive | lands |
|---|---|---|
| The AI pulls files together and offers ideas | agent loop over the catalog with the event graph as context | P5 plus agent loop |
| "Tell it what I'm interested in and it makes things from my stuff" | presets and workflows (`2026-08-25-extensibility.md`) with a standing interest as a parameter; output is a file in the project source | after presets |
| A dossier on a person generated for fun as a PDF | a workflow: gather `person` edges, segments, artifacts; render; write the file into the project | after the above |

## The hidden folder

The store and sidecars living on the medium is decided. Whether that
directory is hidden is not, and the risk is real: a person drags the photos
folder to a new drive in Finder, the dotfolder stays behind, and the
assertions are on the old drive.

Three things reduce it and none removes it:

- Assertions are keyed by content uuid. When the old drive is seen again, or
  the store is found by any path, they rejoin the files by hash. Separation
  is recoverable rather than fatal, which is what content identity is for.
- Spacedrive's own copy and move carry the store. The risk is only outside
  the app.
- The app-owned source exports to plain files (the Markdown vault form) that
  a naive copy does pick up, because they are not hidden.

What remains is a naming decision: a visible directory with an obvious name
(`Spacedrive/` beside the files, or a bundle-style `<name>.spacedrive`) against
a dotfolder that stays out of the way. The archival stance argues visible.
Leaning visible, with the walker hiding it as a lens the way it hides
bundle internals.

## Version control without git

The knowledge layer changes constantly; the evidence never does. So the
version control the corpus gets from git is really two things, and only one of
them is worth building.

- **Evidence is immutable by convention** and content-addressed. Nothing to
  version; a corrected file is a new file, which is already the rule.
- **Assertions need history**, because a person and several agents write
  them and a wrong correction must be reversible. `record_overlay` today is
  last-write with an `hlc` and a `device_uuid`, which is sync bookkeeping
  and not history. An append-only assertion log, one row per write with
  actor and time, gives time travel over the knowledge layer and is the same
  structure sync needs to ship deltas. `source-durability.md` already asks
  the assertion tables to reserve for sync; this is that reservation with a
  second use.

Git stays as an export target for people who want it, via the vault form.

## The order

Stated against what the corpus needs most and what the branch can do now.

1. **The search that takes an hour today.** Map every drive and the NAS,
   snapshot them, and answer a filename query across all of them with the
   drives unplugged. This is the acceptance test for `2026-08-27-storage-map.md`
   and P2.7, both of which landed in the last week, and it is the thing the
   corpus work keeps stalling on. Run it before anything below.
2. **Schema declarations in P2.5.** The core nouns, the `event` facet with
   temporal precision, `edge.provenance` and `edge.recorded_at`, the
   assertion log reservation. Declared now so nothing written during the
   migration is shaped wrong.
3. **The teardown continues.** P3 one registry, P4 delete entries. This is
   the migration path and it does not move.
4. **First enricher: the video bundle.** Sibling and directory convention,
   `web_video` facet, Inspector panel, `segment` from scenes. The data is
   already on disk, so this is the cheapest proof that facets, enrichers and
   the Inspector are one system.
5. **Photos as a file-backed adapter**, faces and places as enrichers,
   `person` cards in the Explorer.
6. **The app-owned source**, the timeline view, temporal propagation.
7. **Transcription and video understanding** as registered kinds with
   presets and native dependencies.
8. **The Story Studio surface** and the generative workflows on top.

Items 1 through 3 are this month. Items 4 onward wait for the teardown for the
reason every plan on this branch gives: a manifest, a facet, or a view built
over a moving substrate gets built twice.
