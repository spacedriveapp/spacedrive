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

The client applies an edit by running `edit_roster` through
`extensions.run_job` with one of these states:

| `action` | Fields | Effect |
| --- | --- | --- |
| `name` | `person`, `name` | Names the person and tags its files `Videos/People/<name>`. An empty name clears it. |
| `hide`, `show` | `person` | Hides a person from the roster or shows it again. |
| `merge` | `person`, `into` | Folds one person into another. |
| `assign` | `person`, `face` | Pins a face to a person. |
| `exclude` | `person`, `face` | Takes a face off a person for good. |

## The view

`ui_manifest.json` declares one library view, `ui/videos.js`. The client lists
it in the sidebar under Extensions. It shows each file's shots with the
keyframe, the description, the places, the people and the transcript lines that
fall inside the shot, and lets you browse shots by person, place and tag. It
reads the `Footage` and `Person` models through `extensions.models` and the
core's sidecars through the sidecar route.

## Building

```bash
cargo build --release
cp target/wasm32-unknown-unknown/release/videos_extension.wasm videos.wasm
```

Install by copying `manifest.json`, `ui_manifest.json`, `videos.wasm` and `ui/`
into `<data dir>/extensions/videos/` of a daemon built with the `wasm` feature.
