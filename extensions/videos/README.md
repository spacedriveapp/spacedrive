# Videos extension

`com.spacedrive.videos` turns a folder of footage into shots you can browse by
person, place and tag. It sits on the core's media sidecars the way Photos sits
on EXIF: the core cuts proxies, transcribes speech and splits each video into
shots with one keyframe each, because a WASM guest cannot run FFmpeg or Whisper.
This extension reads those sidecars and adds what models say about each shot.

## What it does

The `analyze` job reads every video the manifest's glob grants. For each one
it reads the core's shot list (`shots/default.json`, keyed by the record's
`media_uuid`). For each shot it sends the keyframe to `ai.infer` three times:

| Category | Task | Written as |
| --- | --- | --- |
| `image_caption` | `caption` | the shot's description in the `scene` sidecar |
| `scene_classification` | `classify` | place labels in the `scene` sidecar, and `Videos/Places/<label>` tags |
| `face_detection` | `detect_faces` | boxes and embeddings in the `faces` sidecar |

Face detection returns each face's embedding when a `face_embedding` model is
installed, which is why the manifest grants that category too.

A place is a SigLIP scene label: whatever label the `scene_classification`
model ranks highest for the keyframe. The skeleton keeps that label as the
place and does not map it onto a curated place list.

A category without a provider answers `not_available`. The job warns once per
category, records it on the file's `Footage` row, and completes. A video the
core has not split yet is listed as `waiting_for_shots`. Running the job again
fills in only what is missing, so installing a model later and re-running is
enough.

After the pass, `analyze` clusters every embedded face into people. You can run
`cluster_people` on its own, too.

## The roster

People are clusters of faces. Your names and corrections are kept as
`RosterEdit` rows, an append-only log that is the extension's assertion layer:
nothing can rebuild it, so it is never derived from the clusters. Every
clustering pass seeds from the people that already exist, then replays the log.
A named person keeps its id, and so its name, for as long as faces match it.
Each edit names a face by its media uuid, shot id and index in the shot. All
three come from the bytes, so an edit rebinds on another device that holds the
same footage.

Each edit carries an `hlc` and a `device_uuid` from the host's
`assertions.stamp`, the clock tag assertions use. Replay runs in HLC order with
the device as the tiebreak, so when sync merges two devices' logs the edits
interleave in the order they were made. A replay reads the whole log, however
long it gets.

The client applies an edit by running `edit_roster` through
`extensions.run_job` with one of these states:

| `action` | Fields | Effect |
| --- | --- | --- |
| `name` | `person`, `name` | Names the person and tags its files `Videos/People/<name>`. An empty name clears it. |
| `hide`, `show` | `person` | Hides a person from the roster or shows it again. |
| `merge` | `person`, `into` | Folds one person into another. The edit records the faces `person` had, and they stay pinned to `into`, even when the two clusters look nothing alike. |
| `assign` | `person`, `face` | Pins a face to a person. |
| `exclude` | `person`, `face` | Takes a face off a person for good. |

## In the app

The extension has no page of its own. Its output shows where the rest of a
file's metadata does, in the explorer. Selecting a video puts a Footage section
in the inspector with the shot keyframes, each shot's description, an excerpt
of the transcript and the Analyze actions. The explorer can show duration, shot
count and a transcript marker as list columns and grid badges, and the Videos
item in the sidebar lists every video in the library. They read the core's
`media.sidecars` and `media.footage.summaries` queries, which include the
descriptions this extension writes to its `scene` sidecar.

## Building

```bash
cargo build --release
cp target/wasm32-unknown-unknown/release/videos_extension.wasm videos.wasm
```

The desktop app bundles this folder. A release build of the app installs it into
`<data dir>/extensions/videos/` on launch, and replaces the installed copy
whenever the bundled one differs, so an app update also updates the extension.
The release workflow rebuilds `videos.wasm` from the same commit before it
bundles it. The app's daemon is built with the `footage` feature, which
includes the `wasm` runtime.

To try a local build with any other daemon built with `wasm`, copy
`manifest.json` and `videos.wasm` into
`<data dir>/extensions/videos/` and restart the daemon. A dev build of the app
does not install bundled extensions, so it never overwrites that copy.
