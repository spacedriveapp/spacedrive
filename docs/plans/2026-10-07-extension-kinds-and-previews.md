# Extension File Kinds and Previews

> Status: proposed 2026-10-07; nothing built
> Captured: 2026-10-07 (SPAC-31), from the tree at `b938526`
> Owns: how an extension's manifest declares file kinds, how identification
> assigns them, how the client renders a preview for them, and what happens
> to those kinds when the extension goes away
> Register: `PROJECT_STATUS.md`, row "Extension file kinds and previews"
> Companions: `2026-08-30-extension-kinds.md` (adapters, enrichers, apps),
> `docs/core/design/extension-points.md` (the renderer registry and open file
> kinds are its phase 3), report SPAC-28 gap 9 (nothing in the SDK or the
> manifest registers a kind or a preview)

## Outcome

An extension ships a `manifest.json` that names file kinds: a kind name, a
parent kind from the built-in set, the extensions and magic bytes that
identify it, and how to preview it. The daemon folds those kinds into
identification, so a `.cr2` on a drive reads as `raw` under `image` instead of
a bare `image`. The client looks a preview up in a renderer registry keyed by
kind, with the built-in renderers registered first, instead of the switch in
`ContentRenderer.tsx`. A file keeps its kind name after the extension is
unloaded, and the client falls back to the parent's renderer until the
extension returns. This is the second half of product sequence step 5.

## Where it stands

Read from the code on 2026-10-07.

- `core/src/filetype/registry.rs`: `FileTypeRegistry::builtin()` is a
  process-wide `OnceLock` built from fifteen embedded TOML files. Each
  `FileType` has an id (`image/jpeg`), extensions, MIME types, magic byte
  patterns, a `category: ContentKind` and a priority. `ContentKind`
  (`core/src/domain/content_identity.rs`) is a closed `repr(i32)` enum of 27
  values with a `Type` derive, so `ts-client` carries it as a string union.
- Kinds are assigned by extension only. `identify_by_extension` runs in the
  walk (`ops/indexing/arena.rs:398`) and the result travels on
  `ArenaChange.content_kind`, but the store writer never persists it: the
  `content.kind INTEGER` column exists (`crates/store/src/record.rs:98`) and
  `ContentIdentity.kind` is always `None` (`ops/indexing/content_identity.rs:253`),
  so every row is `NULL`. The magic byte path in `identify()` is not called
  from any indexing phase.
- The client sees a kind because `File::from_entry` (`domain/file.rs:290`)
  recomputes it at read time from the name through the built-in registry,
  and `getContentKind()` in `ts-client/src/fileUtils.ts` prefers
  `content_identity.kind` then `content_kind`. Search (`store_search.rs:149`,
  `arena_search.rs:571`, `media.rs:360`) and organize (`organize/plan.rs:287`)
  do the same, eight call sites in all.
- `ContentRenderer.tsx` switches on `getContentKind(file)` over image, video,
  audio, mesh, document, book, spreadsheet, presentation, text, code, config
  and default. `QuickPreviewFullscreen.tsx:93` and `FileInspector.tsx:538`
  switch on the same value for controls and metadata panels. `Thumb.tsx`
  picks an icon from `getFileKindForIcon`, which is the kind name
  capitalized.
- The manifest (`infra/extension/types.rs`) knows `id`, `name`, `version`,
  `wasm_file`, `permissions` and `config_schema`. `extensions.list` returns
  id, name, version and jobs. `extensions/photos/ui_manifest.json` declares
  `file_viewers` keyed by MIME type with a `component` name, and nothing
  reads it.
- The daemon's HTTP server (`apps/server/src/main.rs:567`) serves sidecars
  and hot thumbnails by URL; `ServerContext.buildSidecarUrl` builds them.

So the kind is a derived fact today, computed from the file name every time a
row is read, and nothing is stored. That makes the migration small and the
persistence a new requirement, not a change to an existing one.

## Design

### Manifest schema

`manifest.json` gains one optional section, `kinds`:

```json
"kinds": [
	{
		"name": "raw",
		"display_name": "RAW photo",
		"parent": "image",
		"extensions": ["cr2", "nef", "arw", "dng", "raf", "orf"],
		"mime_types": ["image/x-canon-cr2", "image/x-nikon-nef"],
		"magic": [{ "pattern": "49 49 2A 00", "offset": 0 }],
		"preview": { "renderer": "image" }
	},
	{
		"name": "lightroom-catalog",
		"display_name": "Lightroom catalog",
		"parent": "database",
		"extensions": ["lrcat"],
		"preview": { "viewer": "catalog_summary" }
	}
]
```

Rules, enforced when the manifest is parsed:

- `name` is lowercase `[a-z0-9-]`, unique within the manifest. The kind's
  id everywhere else is `<extension id>:<name>`
  (`com.spacedrive.photos:raw`), the convention `job_registry.rs` already
  uses for jobs. A short name never collides across extensions because the
  id carries the namespace.
- `parent` is one of the built-in `ContentKind` values other than `unknown`,
  `model_entry` and `memory`. A kind has exactly one parent and the parent is
  always built in; no kind may parent another kind in this slice. That keeps
  fallback one step long and keeps the parent a value every existing
  consumer (search filters, kind stats, icons) already understands.
- `extensions` is required and non-empty, lowercase, no leading dot.
  `mime_types` and `magic` are optional. `magic` uses the pattern and offset
  shape the built-in TOML uses.
- `preview` is one of `{ "renderer": <built-in name> }` with a name from
  `image`, `video`, `audio`, `mesh`, `document`, `text`, `default`, or
  `{ "viewer": <id> }` naming an entry in `ui_manifest.json`. Absent means
  the parent's renderer.
- `ui_manifest.json` `file_viewers[]` entries gain `id` and `bundle` (a
  path inside the extension directory to one ES module) beside the existing
  `component`. The binding runs one way: a kind names its viewer through
  `preview.viewer`, and a viewer lists no kinds. The existing `mime_types`
  field stays for the UI contributions brief and does not bind previews.
  Core parses only `file_viewers` from that file; the other sections stay
  unread until that brief.

### Identification

`FileTypeRegistry` stops being a single static and becomes two layers: the
built-in registry (unchanged, still the `OnceLock`) and a process-wide
current registry that the plugin manager swaps when an extension loads or
unloads. `FileTypeRegistry::current()` returns an `Arc` to the layered
registry. `FileTypeRegistry::current()` has two kinds of caller to move:
the five literal `builtin()` calls (`domain/file.rs:240` and `:296`,
`organize/plan.rs:286`, `arena.rs:294` and `:460`) and the
`ActionContext.file_type_registry` field (`core/src/context.rs:112`), a
separate `Arc::new(FileTypeRegistry::new())` that search and the media query
read through `context.file_type_registry()`. The field goes away and the
accessor returns `current()`, so a kind filter and a preview never disagree
about the same file. Lookups stay allocation-free; the swap happens a handful
of times per process.

An extension kind becomes a `FileType` with `id = <extension id>:<name>`,
`category = parent`, priority 110, and a new field `kind_name:
Option<String>` set to the id. Built-in types keep `kind_name = None`.

- `identify_by_extension` picks the highest priority candidate, so a kind
  that refines a built-in extension (`.dng` is `image/x-adobe-dng` today) wins
  by priority. A kind may only claim an extension the built-in table maps to
  its own parent, or an extension the built-in table does not map at all. A
  claim on `.pdf` with parent `image` is rejected at load with a logged
  reason. Refinement is allowed; redefinition is not.
- Magic bytes run in the content identity phase, which already opens every
  file to sample its hash. For a file whose extension is claimed by an
  extension kind with `magic` patterns, or whose extension is unknown, the
  phase reads the first 8 KB (`MAX_MAGIC_BYTES`) and checks the extension
  kinds' patterns only; the built-in patterns stay unused as today. A match
  confirms or picks the kind; no match keeps the extension result.
- The content identity phase writes `ContentIdentity.kind` (the parent as
  the existing `content.kind INTEGER`) and a new `content.kind_name TEXT`.
  Store schema version 2 adds the nullable column; `ContentIdentity`,
  `Entry` and `File::from_entry` carry it through.
- `File` gains `content_kind_name: Option<String>`. `content_kind` keeps its
  type and meaning: the parent for an extension kind, the kind itself for a
  built-in. Nothing that reads `content_kind` changes.

### Surviving unload

The row keeps `kind_name` and the parent `kind`. On read, `File::from_entry`
reports both whether or not the extension is loaded. The client resolves the
name against the kinds `extensions.list` currently reports; a name with no
match falls through to the parent's renderer and the parent's icon, so a
`raw` file previews as an image until Photos comes back. Nothing is rewritten
on unload. When the extension loads again, the same names resolve again.

A kind renamed or removed in a later manifest version leaves rows with a stale
name, which behave like an unloaded extension's rows; carrying them over is a
reidentification pass (below), not a migration.

### Conflicts

Two loaded extensions claiming the same file extension is the only conflict
the id scheme leaves. The rule is deterministic and reported, not interactive:

- At load, a claim on an extension another loaded extension already claims
  is dropped from the registry's extension map. The extension loaded first
  keeps the claim. `load_all` (`infra/extension/manager.rs:176`) walks
  `read_dir` in raw order today; K1 collects and sorts the directory names
  before loading, so the winner is the same on every machine.
- Dropped claims keep their full `FileType` in a `contested` list on the
  layered registry, beside the conflict record `extensions.list` returns as
  `conflicts: [{ extension, kind, claimed_by }]`, so the Extensions page can
  show them and the identity phase can still read their patterns.
- Magic bytes break a tie only when both claimants declare patterns and
  exactly one matches. The content identity phase checks the winner's
  patterns and the contested patterns for that extension; a lone match on a
  contested kind assigns that kind for that file.

A picker in settings is a later slice if people hit this; the data to drive
it is already in the list output.

### The client renderer registry

`packages/interface/src/components/QuickPreview/renderers.ts` owns a module
level registry:

```ts
type Renderer = (props: ContentRendererProps) => ReactNode;
registerRenderer("image", ImageRenderer); // built-ins register at import
resolvePreview(file, kinds): { renderer: Renderer } | { bundle: string }
```

Resolution order: `file.content_kind_name` matched against the kinds from
`extensions.list` (`preview.renderer` names a built-in, `preview.viewer`
names a bundle), then `file.content_kind` against the built-ins, then
`default`. `ContentRenderer` becomes the directory check plus one
`resolvePreview` call. `QuickPreviewFullscreen` and `FileInspector` keep
switching on `content_kind`; they only need the parent.

`useExtensionKinds()` wraps the existing `extensions.list` query and exposes
the kinds as a map; it is the client's only source of extension kinds, which
is what makes unload a no-op on the client.

A bundle preview mounts through a `BundleRenderer` host component. The
module is loaded with a dynamic `import()` from
`/extension/<id>/<bundle>`, a new static route beside `/sidecar` on the
daemon's HTTP server that serves files from the extension's directory and
nothing else. The module exports `mount(el: HTMLElement, ctx) => () =>
void`, where `ctx` carries the `File`, the original byte URL and the sidecar
URL builder. A plain DOM contract keeps the bundle free of the app's React
copy and version; the host handles unmount and errors, and a bundle that
throws falls back to the parent renderer.

### ts-client

Regenerated, not hand edited:

- `File.content_kind_name: string | null`.
- `ExtensionInfo.kinds: ExtensionKindInfo[]` with `id`, `name`,
  `display_name`, `parent: ContentKind`, `extensions`, `preview`.
- `ListExtensionsOutput.conflicts: KindConflict[]`.
- `PreviewSpec` as a tagged union of `renderer` and `viewer`.

`ContentKind` itself does not change. `getContentKind()` keeps returning the
parent; a new `getContentKindName(file)` returns the extension kind id or the
parent name, for the inspector's kind label and the file kinds page.

### Migration

There is nothing stored to migrate: `content.kind` is `NULL` on every row
today and kinds are derived at read time. Store schema version 2 adds
`kind_name` with no backfill. From then on:

- Files indexed after the change get `kind` and `kind_name` at the content
  identity phase.
- Files already in a store keep deriving a parent at read time through the
  layered registry, so a `.cr2` shows `raw` through `extensions.list`'s
  extension table as soon as Photos loads, without a write.
- Loading an extension with kinds for the first time, or with a changed
  extension table, runs one reidentification pass per open store: one
  `UPDATE content SET kind_name = ?, kind = ? WHERE kind_name IS NULL AND id
  IN (SELECT r.content_id FROM record r JOIN facet_file f ON f.record_uuid =
  r.uuid WHERE f.extension IN (...))`. The extension lives in
  `facet_file.extension` (`crates/store/src/file.rs:563`), which has no
  index, so schema v2 adds `idx_facet_file_extension` with the column.
  Magic-only claims wait for the next content identity pass over those
  files. The pass is idempotent and logged; it is not a job.

The built-in TOML definitions and `ContentKind` are untouched, so a daemon
without the `wasm` feature behaves exactly as today.

## Phases

| Phase | Work | Exit proof | Size |
| --- | --- | --- | --- |
| K1 Manifest and registry | `kinds` parsed and validated; layered `FileTypeRegistry::current()`; five `builtin()` call sites and the context field moved; `load_all` sorted; conflicts and contested claims recorded; `extensions.list` reports kinds and conflicts; ts-client regenerated | Unit tests: a manifest with a bad parent or a redefining claim is rejected with the reason; two fixtures claiming `.xyz` load with one conflict in the list output; `identify_by_extension` on `foo.cr2` returns the `raw` type with parent `image` while the fixture is loaded and `image` after `unload` | 2 days |
| K2 Persisted kinds | Store schema v2 `kind_name` and `idx_facet_file_extension`; content identity phase writes `kind` and `kind_name` and checks extension-kind magic; `File.content_kind_name`; reidentification pass on load | Acceptance test (wasm group): index a folder with `.cr2` files under a fixture extension, read the store, both columns set; unload the extension, re-read through `files.list`, `content_kind_name` still `raw`, `content_kind` is `image`; a `.cr2` whose bytes carry no TIFF header and whose extension is also claimed by a second fixture with matching magic gets the second kind | 2 days |
| K3 Client registry | `renderers.ts`, `useExtensionKinds`, `ContentRenderer` becomes a lookup; `getContentKindName`; inspector and file kinds page show the name | `tsc` clean; Playwright or desktop recording of Quick Preview on a `raw` fixture rendering through the image renderer, then falling back to the same renderer with the extension removed from the data dir and the daemon restarted | 1.5 days |
| K4 Bundle previews | `ui_manifest.json` `file_viewers` parsed for `id` and `bundle`; `/extension/:id/*path` route; `BundleRenderer` with the `mount` contract; a tiny fixture bundle in `extensions/test-extension/ui/` | Acceptance: the fixture declares a kind with `preview.viewer`; Quick Preview mounts the bundle and the bundle's DOM shows the file name; deleting the bundle file makes the preview fall back to the parent renderer with one logged warning | 2 days |
| K5 Photos | Photos' manifest declares `raw` (and `heic` where the built-in table lacks a magic pattern) with `preview.renderer: image`; `ui_manifest.json` `file_viewers` gains `id` and `bundle`, with `raw` pointing at it once a viewer exists | Photos loads with no conflicts on a daemon with the built-in table; `sd-cli op extensions.list` shows its kinds | 0.5 day |

K1 and K2 are core; K3 and K4 are client plus one server route; K5 is
manifest edits. K3 can start after K1. Eight days, two PRs (K1+K2, K3 to K5).

## Decisions for Jamie

1. Parent is always built in, and a kind is one level deep. The alternative,
   kinds parenting kinds, gives a richer taxonomy at the cost of a resolution
   walk on every read and a fallback chain the client must know. The built-in
   set covers every parent Photos, a DAM, or a transcription enricher needs.
2. Conflicts resolve by load order and are reported, not asked. The
   alternative is a picker in the Extensions page in the same slice, about a
   day more and a settings key. Recommended: ship the report, add the picker
   when a real pair of extensions collides.

## Open

- Whether `content.kind` should also be written for built-in kinds at the
  content identity phase, ending the read-time derivation. It is cheap once
  K2 writes the column anyway, and it would make kind stats come from the
  store instead of the name. Not needed for this slice.
- Whether a bundle preview should receive byte ranges rather than a whole
  file URL. The original URL already supports range requests for video; the
  `ctx` can grow a `range(start, end)` helper when a timeline-shaped viewer
  asks for it.
- Whether a kind may ship its own icon. `Thumb.tsx` resolves icons by name
  today; a kind-to-icon map in `ui_manifest.json` is the next field.
