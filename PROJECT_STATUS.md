# Project Status

> Last updated: 2026-11-24
> Code baseline: `b1a58be`
> Branch of record: `main` (fast-forwarded from `sources` on 2026-10-05)
> Purpose: short-term working context across development sessions while
> `.tasks/` is frozen.

Read `AGENTS.md` first for repository rules. Read this file next for the current
state and immediate work. Design documents own rationale and long-range phase
plans. This file owns what is true now, what is being worked on, and what the
next session should do.

The product boundary lives in
[`docs/core/product-direction.mdx`](docs/core/product-direction.mdx). Spacedrive
is open personal data infrastructure that makes owned storage competitive with
cloud convenience. It protects a person's ability to choose where files and
metadata live as automated attacks and intelligent systems increase the cost of
centralized custody. Its cross-platform filesystem runtime provides the same
operations across local files, remote nodes, drives, and optional clouds.
Reliability, lightweight distribution, extensibility, and user ownership decide
scope.

## Current focus

NAS operating state (its datasets, deploys, backups and evidence) is
kept privately in the company repo. This file records engineering state.

Development focus is the product sequence below, starting with running
Spacedrive on the Mac and operating a paired NAS's sources from it.

The drive-catalog and physical-drives plans are committed (`2d380fd26`), but
the early `packages/drives` implementation described by previous handoffs is
no longer in the checkout. See the register row before restarting that work.
Physical-drive identity remains separate from product recognition and visuals.

## Immediate register

Beta date and where beta.1 is cut (Capy FAN-14810, November 27, in PR). Jamie
moved the Spacedrive 2.0 Beta target to December 5, 2026 on 2026-11-27.
beta.1 is tagged from a verified `main` head. The `release/beta.1` branch is
retired, so fixes land on `main` and are not cherry-picked. Every in-flight
feature, footage included, ships in beta.1 by merging to `main` before the
cut. The README, the releases page, the introduction and the launch plan state
the new date. No workflow names `release/beta.1`. Docs only; no tag, version
or workflow changed.

Job shutdown deadlock (Capy FAN-12312, November 25, in PR). The Windows unit
job kept failing on `a_directory_that_never_answers_does_not_hold_up_the_pass`.
The cause was not the reconcile pass but `core.shutdown()` at the end of the
test. `JobManager::pause_job` held the running-jobs read lock while it waited
for the job to reach an interrupt check. Content identification dispatches the
text index pass on its way out, and that dispatch needs the write lock, so
shutdown and the job waited on each other. Pause, cancel and the shutdown loop
no longer hold the lock across the wait.
`shutdown_finishes_when_a_paused_job_dispatches_another` reproduces it every
time on the old code. The reconcile test's cloud listing now waits on a gate
instead of a 5 second sleep checked against a 3 second limit. Next: three
green Windows unit runs on the PR.

UI lag while jobs run (Capy FAN-14236, November 25, in PR). On a library
with years of background passes, `jobs.list` returned every job ever run
(17,858 jobs, 7.9 MB on one Mac), and the always-mounted jobs provider
refetched it on every job start and finish. A web reproduction with 17.5k
seeded jobs and 24 shot-detection jobs spent 17 s of 30 in long tasks
while scrolling a grid. The sidebar now reads job counts from their own
context (#3278) and each Tauri event subscription gets its own channel
(#3279). In PR: the job list keeps the newest 200 finished jobs and
coalesces its refetches, and grid cards stop reading dnd-kit's public
context. With all four, the same run has no long tasks. Next: confirm on
the desktop build while footage jobs run.

Sidebar source rows (Capy FAN-14228, November 25, landed in #3275, follow-up
in PR). A source row draws its drive with the Spacedrive icon for its volume
kind (generic drive, external HDD, network server, virtual, cloud provider) and
shows offline, unmounted or locked as a muted trailing icon with a tooltip
instead of a text pill. `sources.list` returns each source's `volume_type`
from its volume row, so an unplugged drive keeps its icon; `volumes.list`
omits this device's offline volumes and cannot supply it. The follow-up draws a
replica as its owning device's form factor (laptop, desktop, phone, tablet,
server), with a server when that is unknown. Next: review and merge.

Acceptance runner losses (Capy FAN-11240, November 24, in PR). About four
in ten runs of the acceptance job lost their Blacksmith runner with no log.
The VM was not short of memory or disk: a monitor that posted samples as
commit statuses showed more than 14 GB free and no OOM kills up to the moment
it went silent. Looping each loop-volume suite on its own pair of Blacksmith
runners lost both runners on `disk_full_acceptance_test` within 20 minutes,
while locked volumes, offline copy, track on a full data disk and a control
set survived 44 to 131 runs each. The disk full suite now runs in the hosted
`zfs` job through `xtask test-core --hosted`. Next: three green acceptance
runs on the PR.

Footage actions in the app (Capy FAN-13825, November 24, beta footage scope).
Videos and folders get Transcribe, Make proxy, Detect shots, Analyze and
Export timeline in the context menu and the inspector, with per-file state
(transcript, proxy, shot count, analyzed shots) and the progress of jobs started
there. The always-registered `media.footage.capabilities`, `media.footage.status`,
`media.footage.run` and `media.footage.export` ops dispatch to the feature-gated
actions or say what is missing (the build, a Whisper model, the Videos
extension), so the generated client types reach them in every build. The dead
`useJobDispatch` hook, which called the extension-only `jobs.dispatch`, is gone.
Nothing runs automatically.

Transcript in Quick Preview (Capy FAN-13826, November 24, beta footage scope
item 6). `File.sidecars` is never filled, so the old subtitle code never
showed anything. The new library query `media.sidecars` takes one file and
answers its content uuid, transcript segments, largest proxy (with its
modification time as a cache buster) and shot list; Quick Preview asks for it
per previewed file, so the listing pays nothing. The video player shows
subtitles, a transcript panel with click to seek that follows playback, and
shot boundaries on the timeline, and swaps to the proxy at the same position
when the webview cannot decode the original. The audio view reads the same
transcript. Both sidecar HTTP servers now answer byte ranges, which video
playback needs. `ops::media` and the shot list reader are always compiled;
only detection stays behind `video-shots`.

Models settings page (Capy FAN-13824, November 24, beta footage scope). Settings
> Models lists Whisper and the inference catalog by category with size,
license and installed state, downloads with a license confirm, shows per-model
progress, cancels and deletes. It shows the active provider per category and
configures the bring-your-own-key cloud caption client. The `ai` ops, catalog
and caption key store now build without any ai feature, so their types are in
the ts-client; only inference stays behind `ai-cloud` and `ai-local`.

Footage without a system FFmpeg (Capy FAN-13823, November 24, beta footage
scope). Proxies, speech audio, shot detection, keyframes and the clip probes
ran the `ffmpeg` and `ffprobe` executables from the PATH, which a Mac without
Homebrew lacks. They now run in-process on the linked FFmpeg libraries
through `sd_ffmpeg::footage` (probe, scene scores, JPEG frame, transcode),
with the same filter strings and encoder settings, so `video-proxies` and
`video-shots` now turn on the `ffmpeg` feature. The footage unit tests make
their own media with the libraries and need no executable. Video thumbnails
and thumbstrips use the same libraries in builds with the `ffmpeg` feature,
and the host tool only in builds that ship no codecs. The release desktop
daemon builds without that feature outside dry runs, so it reaches users once
the footage features are on in release builds.

Content search in the explorer (Capy FAN-13827, November 24, footage beta).
Explorer search runs `search.content` beside `search.files` over the same
scope, limited to `transcript`, `scene` and `caption`, and lists hits by file
above the file results with their snippets. A timestamp chip opens Quick
Preview at that moment through `openQuickPreview(id, { startSeconds })`, which
the video and audio players honour once metadata loads. Library scope asks
every attached local filesystem source that reads contents, one query each.

Scene and caption search (Capy FAN-12538, November 22). `search.content`
finds files by the descriptions extensions write: the Videos extension's
`scene` sidecar (kind `scene`, one row per shot with its in and out points,
behind `video-shots`) and the Photos extension's `caption` sidecar (kind
`caption`, behind `ai-cloud`). Rows live in `text.db` keyed by content
uuid and are replaced when the host writes or deletes the sidecar; the host
gained `sidecars.delete` and the SDK `delete_sidecar`. No backfill pass yet:
sidecars written before this build are indexed on their next write.

Daemon event broadcaster survives a lag (Capy FAN-12542, November 22, beta.1
investigation of "UI doesn't update after moving files"). The RPC server's one
forwarder from the core event bus ended on the first `Lagged`, which stopped
every client's live updates until the daemon restarted. It now skips the lost
events, sends `Refresh` to each connection that takes it, and keeps going; log
followers skip lagged lines the same way. A reconcile that removed entries
emits one `Refresh`, which now passes resource and path filters, so a view
open inside a folder the rules newly exclude refetches. Jamie moved no files;
the stale view followed the excluded-directory prune below.

Excluded-directory pruning goes through the removal fuse (Capy FAN-12573,
November 22, beta.1 blocker). On the beta.1 MacBook the first startup
reconcile after #3224 removed 2.77M of 5.4M records from the home source. They
sat under directories the rules now exclude (`~/Library`, dev directories,
gitignored folders) that older walks had kept. That prune bypassed the
half-removal fuse. It now counts toward the fuse and waits for a whole read,
like any removal, so a pass that would take more than half of a source
removes nothing and logs `reconcile refused its removals`. Tag assertions on
the removed records survive as orphans and rebind if the records return.
Proof: `an_exclusion_that_would_take_most_of_the_source_removes_nothing`.
Open: a source whose excluded content outweighs the rest is never pruned.

Footage ships in the desktop app (Capy FAN-13822, November 24, beta footage
steps 1 and 3). sd-core has a `footage` umbrella feature (`ai-local`,
`ai-cloud`, `footage-clips`, `speech-to-text`, `video-proxies`,
`video-shots`, `wasm`).
`release.yml` builds the desktop daemon with it on every run, tagged or dry,
on macOS arm64 and Linux x86_64 and aarch64; a dry run's `features` input
adds more on top. The Intel Mac daemon gets `footage-without-onnx` (`ai-cloud`
without `ai-local`): ONNX Runtime stopped publishing Intel macOS builds after
1.23 and ort rc.15 has no prebuilt binary for x86_64-apple-darwin, so local
categories answer not_available there while bring-your-own-key cloud captions
work. The standalone CLI, nightly and server binaries keep
their feature sets: the CLI and nightly ship no codec framework for
speech-to-text to link, and nightly builds no desktop app. Linux now takes ONNX
Runtime from ort's download, as macOS does; the old `libonnxruntime.so` in
apps/.deps no longer links. The app bundles `extensions/videos` (wasm rebuilt
from the same commit in the release job) and a release build installs or
updates it in `<data dir>/extensions` before it starts the daemon. Every
footage job is still an explicit action.

Videos extension skeleton, footage brief 6 (Capy FAN-12393, November 22, off
the beta.1 path). `extensions/videos` (`com.spacedrive.videos`) reads the
core's shot lists and keyframes, sends each keyframe to `ai.infer` for a
caption, place labels and faces, and writes `scene` and `faces` sidecars. Faces
cluster into a people roster whose names and corrections are an append-only
edit log in the extension's store, stamped with an HLC and device uuid from the
host's `assertions.stamp` and replayed in full, in HLC order, after every
clustering pass. The view's Tags tab lists every library tag. Until
the provider layer lands every category answers `not_available` and the job
completes with the shot list browsable. The host side is behind the `wasm`
feature: `sidecars.read_media` for core sidecars, `media_uuid` on records, the
`extensions.models` query, and `ui_manifest.json` `views`, which the client
lists under Extensions in the sidebar. Live inference and real shot lists wait
on the provider layer and the shot detection job.

Transcription job, footage piece 2 (Capy FAN-12387, November 22, behind the
off-by-default `speech-to-text` feature, not on the beta.1 path).
`media.transcribe` runs Whisper over the audio and video under a path
(`core/src/ops/transcription`): it reads the speech audio sidecar or streams
the original through `sd_ffmpeg::AudioStream`, transcribes only the voiced
stretches of each two-minute chunk, and writes `transcript/default.{json,srt,vtt}`
on the media sidecar layout. Segments land in `text.db` as their own FTS rows,
so `search.content` and the agent `search_content` tool return a hit per
segment with its start and end seconds (`docs/core/transcription.mdx`).
Proven on Linux with generated speech; Metal acceleration and the desktop app
are unverified. `media.transcribe` is the only trigger.

Video proxies and speech audio (Capy FAN-12386, November 22, not beta.1).
The `media.proxies.generate` action, compiled only with the `video-proxies`
cargo feature (off by default), runs a resumable job over a video file or
folder. For each video it writes an H.264 proxy (720p by default, 1080p on
request, a keyframe every second, frame timing and start timecode passed
through) and the first audio track as 16 kHz mono 16-bit WAV, using the
FFmpeg libraries the app links (no `ffmpeg` executable since FAN-13823). Both live in the content-addressed sidecar tree keyed by the
candidate content uuid (`sidecars/content/../proxies/720p.mp4`,
`audio/16k_mono.wav`), so a rerun or a moved file does nothing; the layout
is under "Media sidecars" in `docs/core/virtual-sidecars.mdx` and resolved by
`ContentSidecar`. The job checkpoints after each file and resumes after a
killed daemon; on Linux FFmpeg dies with the daemon. Transcripts, shots,
descriptions, faces and clip export read these files.

Inference providers behind `ai.infer` (Capy FAN-12388, November 22, off the
beta.1 path). The `ai-providers` cargo feature, off by default, adds
`core/src/ops/ai`. It is the union of `ai-cloud` (the registry, `ai.infer`
and the cloud caption client, no ONNX Runtime) and `ai-local` (the ONNX
providers): a registry per category (face detection, face embedding,
scene classification, image caption, text embedding), local ONNX models
downloaded through `ModelDownloadJob` with a pinned hash after the user
accepts the license (`ai.models.download`), batched `ai.infer` requests, and a
per-library bring-your-own-key cloud caption opt-in, off by default
(`ai.cloud_caption.configure`). Photos gets face boxes with embeddings and
scene labels from a daemon built with `wasm,ai-providers`. The catalog pins
BlazeFace, AuraFace, SigLIP and E5 (approved November 22); BlazeFace's ONNX
conversion is hosted at `spacedrive/spacedrive-models` on Hugging Face,
pinned to a revision.
Local captioning runs Florence-2 base (8-bit ONNX, 275 MB, MIT): vision
encoder, BART encoder and a greedy decoder with its key-value cache, so a
video shot's `scene` sidecar and a photo's `caption` sidecar get a
description with no cloud key (Capy FAN-12532).
CoreML is unproven.
See `docs/core/inference.mdx`.

Beta.1 log and churn polish from the signed Studio proof (Capy FAN-12343,
November 21). The desktop app checks the release feed from Rust
(`apps/tauri/src-tauri/src/updater.rs`): a missing or unparseable
`latest.json` is "no update" and logs at info, while network and signature
failures log at warn; the plugin's own updater target is filtered off because
it logs ERROR for a 404 feed. Ephemeral jobs no longer write their status to
the jobs table, which logged ERROR when the row was absent. `JOB_PROGRESS`
lines log at debug. `useJobs` in ts-client reads its callbacks through a ref,
so the desktop app keeps one job-event subscription instead of reopening it on
every progress event. The app and bundled daemon carry the Photos library
entitlement, so tccd can prompt for `Photos Library.photoslibrary`.

Walks move past folders behind unanswered privacy prompts (Capy FAN-12315,
November 21, beta.1 blocker). On first launch macOS asks for Desktop,
Documents and Downloads, and an unanswered prompt blocks the open of that
folder. Discovery now reads each directory on its own task bounded by the
reconcile's `READ_BOUND` (10 s), so the walk finishes and commits everything
else. A folder that does not answer is neither read nor failed: the sweep keeps
what the store holds under it, `VolumeIndex` records it as waiting on access,
and `sources.list` and `core.index_status` report it as `awaiting_permission`
(`sd-cli source list` shows the count). The blocked listing returning is the
signal that the prompt was answered, and it wakes
`reconcile::revisit_awaiting_access`, which reconciles only those folders; a
5-minute timer catches a refusal later allowed in System Settings, and the
startup reconcile reads them too. Only one blocked read per folder is ever
outstanding; a revisit's bound is twice what the last listing took, so a slow
mount answers instead of timing out forever, and a folder outside any source is
revisited under its drive's map. Opening such a folder in the explorer tries
the open with the same bound and returns `awaiting_permission` on
`files.directory_listing`, which the explorer shows as a banner, instead of
hanging. Full Disk Access is not requested and the folders walked are
unchanged. On the Studio a startup walk still stalled at 0% (November 21): the
count a source walk takes of a folder the rules hold back, such as
`~/Library`, read every directory under it with no bound and blocked in other
apps' data. `summary::count_subtree` now reads each directory under the same
bound and leaves out one that does not answer or that another read is blocked
on, holding its read so nothing opens it twice without reporting it. Every
reconcile writes the store's counts to the source row, so `sources.list`
shows the records the startup reconcile added before the walk saves. Proof:
`a_walk_after_a_reconcile_holds_a_folder_still_finishes`.

Read-only FUSE mount on Linux (Capy FAN-12103, November 20). A daemon built
with the `fuse` feature and `mounts.fuse_enabled` on mounts every source, local
and replicated from paired devices, at a path through `sd-cli fuse mount`.
Listings and attributes come from the arenas, bytes from the byte plane, so
peer reads go through the block cache. `--grant` narrows a mount to one
grant's folder, checks `authorize` on every listing and open, and takes the
mount down within five seconds of revocation or expiry. Shutdown unmounts
everything. `core/tests/fuse_mount_test.rs` runs in the acceptance group and,
with `SD_REQUIRE_FUSE`, in the hosted zfs job. Linux release and nightly builds
turn the feature on; it links no libfuse. Symlinks are left out of the mount.
The block cache's L2 write now uses a unique temp per write, since a demand
fetch and a read-ahead storing one block could tear it. The macOS mount stays
the SMB share.
`docs/core/fuse-mount.mdx`.

Content reads never hydrate cloud placeholders (Capy SPAC-86, November 19,
beta.1 blocker). Every path that reads file bytes stats the file first and
refuses one macOS reports `SF_DATALESS`
(`ops::indexing::metadata::refuse_dataless`): content identification, the
integrity tier, thumbnails and thumbstrips, EXIF, the extension record reads
and the byte plane's `LocalProvider`. Hashing records a placeholder with the
`content_error` "cloud placeholder: contents not downloaded" and keeps the
walk's metadata. Hashing reads run on their own runtime at background QoS on
macOS (nice 10 on Linux), and its progress total grows when records arrive
mid-pass. A source walk applies rules and so never sweeps its store, so the
reconcile now removes what the map holds under a directory today's rules
exclude, weighed by the removal fuse since FAN-12573, and the watcher rejects a path when any
ancestor below the root is excluded (`/Users/*/Library` names only the
directory). Open: a placeholder downloaded later keeps its error until the file
changes or its volume remounts; the grep op held on `capy/search-grep` (#3219)
must call `refuse_dataless` before it opens a file; Quick Preview of a local
file loads through Tauri's asset protocol, which the daemon never sees.

Watcher snapshot flushes are paced by snapshot size (Capy SPAC-83, November
19, beta.1 blocker). #3181 rewrote a drive's whole arena snapshot every 30 s
whenever its arena was dirty, which on a 1.8M-record home source with constant
cache churn came to about 11 GB written per hour. The periodic flush now waits
`size / 512 MiB per hour` between saves of one drive, so a 109 MB snapshot is
written about every 13 minutes; shutdown and walk saves still write at once,
and the save runs on a blocking thread. A crash between saves restores the
older snapshot and the warm-start reconcile applies what it missed
(`a_snapshot_older_than_the_last_changes_is_reconciled_on_restart`).

File System Intelligence is a flagship priority identified by James (Codex,
September 18). The product boundary now names persistent file and folder
understanding as a first-class user benefit, with optional cloud acceleration.
The adjacent website's `RELEASE_DIRECTION.md` records a positioning and demo
proposal: understand an unfamiliar drive, find by meaning, retain user context
and keep the saved understanding when disconnected. James chose the name
File System Intelligence and asked to retain the original website hero. Final
copy and supported release scope remain for review. Website presentation is
implemented on the adjacent `beta-release-direction` branch: `/intelligence`, a homepage
section and an interactive sample-drive walkthrough. TypeScript, the production
build and browser interaction checks pass, including mobile layouts and the
restored original hero. No intelligence runtime changed.


Website release direction is captured in the adjacent `spacedrive-web`
repo (Codex, September 17). James's existing rebuild is committed as `74ebe73`
on the new `beta-release-direction` branch. A stand-in 2.0 Beta blog post and
`RELEASE_DIRECTION.md` carry the agreed model commitment into the planned
redesign. The production build passes; HTTP checks verify local draft rendering
and a production 404, with no draft in the blog index or static routes. The
site's false stable-v1 label is corrected. This work does not publish the site
or change core runtime code.

Release identity agreed with James (Codex, September 17): the target is
Spacedrive 2.0 Beta. The alpha period established the direction; beta commits
to the source/library data model and preserving supported beta libraries as
schemas evolve. Keep the 2.0 generation and explain the skipped stable 1.0 and
the rewrite honestly in the launch post. The README and release policy now
name the beta target; `docs/plans/2026-09-17-beta-launch.md` records the rationale
and draft launch wording. The target moved from October 1 to November 1, 2026
on 2026-10-05 (Jamie), to November 15, 2026 on 2026-10-31 (Jamie), to
November 27, 2026 on 2026-11-21 (Jamie), and to December 5, 2026 on
2026-11-27 (Jamie).
This is a release goal, not a claim that the gates have passed. No package
versions, tags, builds or published releases changed.

Alternate shell composition is implemented (Codex, 2026-09-15, uncommitted).
Inset content is the default; Settings > Appearance > Layout retains Floating
panels. The sidebar footer restores circular Activity and Settings controls,
including the activity count and busy indicator. Both modes share mounted
content and stable preview hosts. The local
preference updates other windows on the same origin. Preview and Size view
follow the sheet bounds, and Size thumbnails use container coordinates. Desktop
and web frontend builds pass. Typechecking reports no errors in the changed
files, but the full check still fails on existing dependency and SVG
declarations. Browser control was unavailable in this sandbox. Next proof:
switch layouts from Settings while browsing and playing video, then check Size
view, collapsed panels, inspector popout, light themes, and macOS window controls.

Source read routing is agreed: keep a loaded arena as the fast path when its
coverage and query support suffice; otherwise read SQLite. Select one primary
backend per source request, preserve the same result contract, and never retry
an empty result against the other backend. R6 in
`docs/plans/2026-09-15-source-runtime-reliability.md` records the decision,
including direct local reads before peer database delivery.

September 16 volume audit (Codex): the arena already saves snapshots without
source registration. On the running Mac, Untitled is detected but unmapped and
unwatched, while an older file exists at its expected snapshot path. Startup
maps attached external drives; the hot-plug monitor does not invoke that path.
Do not describe Add to Library as the sole way to persist a filesystem index.
See `docs/plans/2026-09-16-volume-discovery-research.md`. Checks were read-only.

Add to Library UX direction is agreed (Codex, September 16): use that action
everywhere, with one modal, configurable defaults and per-add overrides. Adding
a filesystem path also tracks its containing volume in the selected library;
adding the whole volume uses the same flow with root scope. Untitled remaining
untracked is expected, not a request to auto-add discovered drives. The user
chooses whether the source store lives in the Spacedrive data folder or on the
source under `.spacedrive`. `docs/plans/2026-09-16-add-to-library.md` records the
decision and proposed offline-copy, storage-resolution and setup behavior.
Implementation and the proposed defaults remain for review.

Remote catalog access is agreed: the source's Spacedrive instance opens the
store and serves or replicates it through Spacedrive. A client opens only its
local replica, never the remote live SQLite file through a network mount.
Store placement does not change this access contract.

Add to Library implementation research is recorded in that plan (Codex,
September 16), through `4cf25c17c` plus identified uncommitted database-export
work. Reuse the new read-only store path and exports. Remaining foundations:
consistent volume registration/defaults, library-scoped store resolution,
portable source identity, handle retirement, unconditional managed-directory
exclusions, and removal that preserves portable catalogs. Full offline store
copies are distinct from existing saved arena maps. On-source WAL databases
need supported local storage on their serving daemon. No runtime code or fleet
state changed; placement defaults and handoff/consolidation remain for review.

Library sync and membership research is ready for review (Codex), recorded in
`docs/plans/2026-09-15-library-sync-research.md`. The live Mac and NAS are paired
but hold different library UUIDs, each with one device row and zero eligible
sync peers. Overview storage totals include peer summaries while device counts
stay local. The sync service exists despite the false library setting; source
registration sync, reliable bilateral membership and library-scoped serving
remain gaps. Review the proposed small library-state contract and migration of
the two existing libraries. Evolve the existing `Syncable`/inventory framework:
retain domain-owned apply rules and shared transport, remove entry-specific
coupling, and repair membership and write durability. This research changed
no live membership or config.

The UI setup audit confirmed `SyncSetupModal.tsx` survives without callers:
`b653676e8` removed the overview's Setup Sync button. Pairing still closes
without library setup, and Library Settings only exposes the sync toggle.
The library-sync research now includes restoring setup entry points,
continuing after pairing, and resuming setup for already paired devices.

Locations research is recorded in
`docs/plans/2026-09-15-locations-research.md` (Codex). James confirmed this version
is not backwards compatible. Remove locations without migrating rows, retaining
IDs or adding compatibility APIs. Update current callers to sources, Space items
and policies. The audit found Add Storage calling the pin operation, incomplete
pin sync and duplicate Space shortcuts. Both live libraries returned no pins
during read-only checks. This audit changed no runtime code or live state.

The locations proposal was revised after user review: pins alone duplicate
Space items, while the old requirement to keep selected jobs processing a path
is still needed. T4.4's subtree enrichment-policy replacement has not landed.
James agreed to option 1 for addressing: policy creation accepts `SdPath`,
persists a separate `{ source_id, relative_path }` target, and resolves it to
an execution address through the source layer. Add no source variant to
`SdPath`. Space items own navigation and can reuse the target value. Reuse the
source change stream and job executor. Detailed policy storage and APIs remain
for review; the final-drop handoff has no location compatibility or migration
gate.

Tags on source stores is implemented through P5 (Fable, 2026-09-17,
uncommitted). Definitions and assertions live in each source store per
`docs/core/design/tags-and-assertions.md`; the library keeps only a staging
table for unapplied definitions and the assertion outbox. The old semantic
tag surface (`TagManager`, closure and conflict services, the facade,
`UserMetadataManager`, `ops/metadata`) is deleted, the slim `Tag` is
uuid/path/name/color/icon, and every read serves from stores directly, so a
cold daemon and a detached source both answer. Writes against replicas
owned by another device queue durably, overlay the replica listing
immediately, and deliver on the owner's ack. No legacy migration: zero tag
rows verified on both libraries.
Execution record and remaining P6 items are in
`docs/plans/2026-09-17-tags-on-source-stores.md`. This is source-store
assertion sync; library sync stays deferred.

Path-scoped thumbnail generation is implemented and verified (Codex, 2026-09-15, uncommitted). `thumbs.generate` accepts a file or indexed folder with missing/stale/force modes, uses resumable batches, and versions video tiles against host FFmpeg availability. File and folder context menus plus `sd thumbs generate` call the same operation. Ten focused tests, core/CLI/server/Tauri checks, generated types, CLI help, and the desktop frontend build pass. Changed hooks have no TypeScript errors; the full interface typecheck still fails in existing dependency and SVG declarations. Restart the daemon and reload the app, then regenerate a folder containing an old video icon to verify the live repaint. See `docs/core/thumbnail-generation.mdx` for coverage and resume semantics.

Update this table whenever work starts, lands, becomes blocked, or changes
owner. Keep only active and near-term work here. Move completed context to
Recent landed.

| Work                                              | Status                                 | Owner           | Next proof or action                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ------------------------------------------------- | -------------------------------------- | --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Credentials in logged URLs | in review 2026-11-22 (SPAC-112, FAN-12574); reaches beta.1 through `main` | Opus | A beta.1 daemon log held an SMB URL with its password. `sd_utils::redact::redact_url_credentials` replaces the userinfo of every URL or `//user@host` mount source in a string with `***`. It runs on every log sink (daemon stdout, `daemon.log`, log streams, the log bus that feeds `logs` subscriptions, job logs, sd-server, the Tauri shell, mobile core), on errors the daemon returns to clients, on job error messages, on network volume `hardware_id`, and on the SMB share's startup line. `mounts.status` still returns the credentialed `smb_url` and mount hint, since that query exists to hand them to a local client. |
| Release candidate validation (cloud) | run 2026-11-05 on nightly `26b57e1` (SPAC-58); steps 1 to 4 and 7 run, record in `docs/core/acceptance/candidate-validation.md` | Opus | Steps 1 (Linux), 2, 3 and 7 pass. A v2.0.0-alpha.2 library opens but loses its locations, files, tags and collections (table in the record). Windows clean install fails until a nightly with #3169 is published; then rerun the smoke workflow from #3166. Step 4 fails. Fixed and merged: truncated file left by a killed copy (#3170), `--instance` data dir nesting (#3171), store migrations missing from the log (#3168), Windows verbatim paths (#3169), unhealed first walk (#3172), thumbnail scope (#3174), library statistics (#3173, #3182, #3183), `logs show` path (#3175). Nightly builds cancelled by every push (#3186, merged). In review: CLI panic on a closed pipe (#3188). The heal now resolves registered sources directly, so sources on hidden volumes (Linux `/`, `/home`, overlay roots) heal too (#3192, `hidden_volume_heal_acceptance_test`). Changes made while the daemon is stopped are reconciled at startup by the warm-start reconcile from `zero-onboarding-startup.md` (SPAC-63, `stopped_daemon_reconcile_acceptance_test`; 16 s cold on the 1M-file tree against a 125 s walk). Open: a full disk hangs the indexer. Remaining: macOS installs and signing, real prior-version Mac state, a cross-network pair, remote operations, update signatures and rollback, release notes and checksums. |
| Intel Mac app bundles an arm64 daemon | in PR 2026-11-22 (SPAC-113, FAN-12578); one PR against `main`, reaches beta.1 through `main` | Opus | SPAC-107 found that the Intel desktop build ran `build:daemon:release` for the runner's host arch; the x86_64 app from signed dry run 44904995093 carries an arm64 `sd-daemon`. The sidecar script now builds for `TAURI_ENV_TARGET_TRIPLE` (lipo for universal), `build.rs` stops copying a host daemon under a cross triple, and release.yml fails a macOS build whose app or daemon has the wrong arch. Proof is an unsigned dry run where `lipo` shows x86_64. |
| Startup reconcile on a large macOS home source | in review 2026-11-19 (SPAC-84, FAN-11141) | Opus | On the beta.1 MacBook the startup reconcile of a 1.8M-entry home source ran past 35 minutes with nothing visible. It read every directory the map held, including `~/Library`, which the map kept from walks before the Nov 5 `Users/*/Library` rule, and iCloud listings in it timed out (os error 60). It also checked each new entry with a ruler built from that entry's own path, which skipped `.gitignore` for files, so every pass added gitignored files to the store (10,000 of them on the generated tree); the watcher shared that check and now builds it for the file's directory too. The pass now builds one ruler per directory as the walk does, leaves held directories the rules now exclude unread, never lists a cloud provider placeholder (`SF_DATALESS`; the walk and its summary count skip them too), bounds each listing at 10 s, and runs as a `reconcile` job with progress and an ETA. Every skip logs at info. Next: SPAC-79 relaunch on the MacBook to confirm the time and the `excluded`/`placeholders` counts. |
| Candidate validation follow-ups | in progress 2026-11-05 (SPAC-64); one PR per finding against `main` | Opus | Four findings from the SPAC-58 run. A full disk hangs the indexer: reproduce on a loop-mounted data dir with a debug build, fail the job with a disk-full error and leave the store consistent, with an acceptance test on the loop-device helper. The receive side of a peer transfer writes in place: stage beside the destination and rename on verified completion as #3170 does for local copies, keeping the #3109 resume rules, with a kill-mid-receive case in the two-core transfer suite. `sd daemon install` without systemd or launchd now says so and prints the command to run instead. The README names Windows as a preview platform, matching the beta.1 fact sheet. |
| Footage 3: shot detection | in review 2026-11-22 (SPAC-102, FAN-12391); behind the `video-shots` cargo feature, off by default, not on the beta.1 path | Opus | `media.shots.detect` runs a job that scores every frame of the 720p proxy (or the original) with FFmpeg's scene score, cuts at 0.3 or at an isolated spike above half of that, and keeps every shot at least 1 s long. It writes `shots/default.json` and one `keyframes/<shot id>.jpg` per shot, which `media.shots.list`, `ops::media::shots::read_shots` and the agent MCP `list_shots` read. On a 20-minute 480p vlog it found 143 shots in 12 to 26 s, and a rerun kept every shot id. Next: Jamie's OK to merge, then a rerun on a SPAC-99 proxy once that lands. |
| Footage 5: clips and timeline export | in review 2026-11-22 (FAN-12392); behind the `footage-clips` cargo feature, off by default, not on the beta.1 path | Opus | `media.clips.search`, `media.footage.read` and `media.timeline.export` join the transcript, shots and proxy sidecars of a video into clips with whole-frame in and out points, and write a rough cut as FCPXML 1.10, CMX 3600 EDL or OTIO through the new `sd-timeline` crate. The CLI (`sd media clips search`, `sd media footage read`, `sd media timeline export`) and the agent MCP tools `search_clips`, `read_footage` and `export_timeline` wrap them, scoped to the grant's directory (`docs/core/footage-clips.mdx`). A daemon run on generated 23.976 and 25 fps footage cut a DTD-valid FCPXML whose in frames match the search. Opening the files in an editor is unverified. Next: Jamie's check in the editors. |
| Agent grants core (G1) | landed 2026-11-20 (#3221, FAN-11130) | Opus | A grant lends one directory of a tracked source to an agent: `grants.create/list/revoke/extend/audit`, `sd grant`, tokens stored as SHA-256 and shown once, 5 min to 12 h lifetimes, revocation on the next request. `ops::grants::authorize` is the one enforcement point; it resolves paths by filesystem identity (`..`, symlinks out, firmlink and case aliases) and keeps writable grants off AGENTS.md, CLAUDE.md, SKILL.md and .git. Bounded audit table. Design in `docs/core/design/agent-grants.md`; `agent_grants_acceptance_test` and the sd-cli `grant_test`. The agent MCP HTTP surface (M1, #3223) builds on `authorize`. |
| Track on a full data disk | in review 2026-11-06 (SPAC-68, FAN-5078) | Opus | `sources.track` on a nearly full data disk registered the source, failed to queue its walk, returned success, and the heal never walked it because the root stayed flagged as indexing. The store is now marked walk-unfinished before the walk is queued, a failed dispatch clears the indexing flag, a walk whose store cannot open is refused, and the action returns an error saying the walk will retry at the next start. Proof: `track_full_data_disk_acceptance_test`. |
| macOS Data volume read as unmounted | in review 2026-11-13 (SPAC-73, FAN-6659) | Opus | On macOS 26.1, `/`, `/System/Volumes` and `/System/Volumes/Data` report the same `st_dev`, so the device comparison in `is_mount_point` (from #3151) called the Data volume unmounted and every walk and browse under `/Users` was refused. macOS and iOS now judge a mount point by `statfs` `f_mntonname`; Linux keeps the device comparison. A refused first walk now fails `sources.track` with the reason, and a store kept in the library is marked walk-unfinished so the heal retries. Proof: `statfs_mount_point_tests` (Linux and Mac), `track_refused_walk_acceptance_test` (its macOS case ran on the Mac Studio), and a real daemon on the Studio walking a folder under `/Users`. |
| Case-only renames and firmlink spellings on macOS | in review 2026-11-13 (SPAC-74, FAN-6716) | Opus | SPAC-72 found that on APFS a case-only rename (`Readme.md` to `README.md`, `keep` to `Keep`), offline or live, left the old spelling as a second record, because the reconcile's removal re-check and the macOS watcher decided presence by stat, and APFS answers a stat for any case of a name. Presence is now decided by the name the directory stores (`sd_fs_watcher::present_as_spelled`: `getattrlist` on macOS, the parent listing elsewhere): the reconcile's re-check uses it, the macOS watcher classifies single-path rename events with it and pairs a remove with an earlier create by inode, and the change handler checks it on macOS. Case-sensitive volumes still keep `Readme.md` and `README.md` apart. `file info` resolves a path through the volume's spelling (`VolumeManager::index_spelling`), so `/Users/...` finds the record held under `/System/Volumes/Data/...` and answers in the spelling asked; the CLI makes relative paths absolute, exits non-zero when nothing is found, and prints search results through the firmlink (`short_spelling`). Proof: `spelling` unit tests with case folding and the firmlink table injected, `path_spelling_acceptance_test` (casefold ext4 on GitHub's runner, the temp dir on a Mac), macOS watcher tests, and a daemon on the Studio. |
| Acceptance flakes on main | in PR 2026-11-06 (SPAC-67); one PR against `main` | Opus | Two acceptance tests failed on main without a code change. `wasm_extension_test::extension_job_resumes_after_restart` (2c4da34) read the guest's checkpoint as soon as the job reported Paused, but Paused is reported when the pause is requested and the guest can land one more checkpoint before it stops; resume itself was right. The test now reads the checkpoint after shutdown, which waits for the job's last save. `disk_full_acceptance_test` (bcc2fa9) filled the disk the whole data dir was on, so the walk's job-row insert in `jobs.db` could hit SQLITE_FULL before the walk began; only the source stores now sit on the filled volume. Finding, not fixed: on a real install whose data dir disk is full, `sources.track` can register the source and skip the walk with only a log line, and the heal never walks it once there is room. |
| Disk-full integrity check | in PR 2026-11-20 (SPAC-87, FAN-11245); one PR against `main` | Opus | `disk_full_acceptance_test` failed once on #3219 with `PRAGMA integrity_check` returning "disk I/O error" (code 4874, `SQLITE_IOERR_SHMSIZE`). A connection that opens a WAL store on a full volume has to rebuild the `-shm` index and has no free blocks for it, so it fails before reading a page; the store is not damaged (a copy and an in-place check after freeing space both pass). The test now checks the store as the full disk left it through a copy on the regular disk, and `SQLITE_IOERR_SHMSIZE` counts as disk full, so a salvage stops at the first such row. 100 of 100 runs passed on main and 60 of 60 on the change. |