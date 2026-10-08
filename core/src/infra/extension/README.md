# WASM Extension System

This module runs Spacedrive's sandboxed extensions. An extension is a
directory under `<data dir>/extensions/` holding a `manifest.json` and the
`.wasm` file the manifest names. The daemon loads every such directory at
startup, before any library opens, so a job resumed at library load finds its
extension. A build without the `wasm` feature has no runtime: `extensions.list`
reports `supported: false` and `extensions.run_job` is refused.

## Pieces

- `manager.rs`: `PluginManager` discovers, compiles (wasmer), instantiates and
  keeps each plugin's store and instance for its lifetime. `PluginRuntime`
  runs one job export on the calling thread.
- `host_functions.rs`: the functions a guest imports from the `spacedrive`
  module. `spacedrive_log`, `register_job` and `register_model` serve
  `plugin_init`; the `job_*` functions report into the running job through a
  `JobBridge`; `spacedrive_random` and `spacedrive_now_ms` give the guest
  entropy and wall-clock time; `spacedrive_op` carries every request that
  returns data.
- `ops.rs`: the operations behind `spacedrive_op`, named by a string and
  carried as bytes. `JobOps` holds the library, the manifest and the open
  extension store for the job's duration and answers one request at a time,
  because the guest blocks on each. Permission is checked here against the
  manifest, never in the guest.
- `job_registry.rs`: the jobs extensions registered, keyed
  `<extension id>:<job name>`.
- `model_registry.rs`: the models each extension declared through
  `register_model`. The registry turns them into one `DataTypeSchema` and opens
  an sd-store database at `<library>/extensions/<extension id>/data.db` the first
  time a job touches a model.
- `wasm_job.rs`: the one core job type that runs every extension job. It
  holds the guest's state as a JSON string and persists it through the normal
  checkpoint table, so a kill or a pause resumes from the guest's last
  checkpoint.
- `types.rs`: the manifest. Both the manifest and its `permissions` block
  reject unknown fields, so a typo in a grant fails at load instead of
  silently granting nothing.

## File kinds

A manifest may declare `kinds`: a name, a built-in `ContentKind` parent, the
file extensions and optional MIME types and magic bytes that identify it,
and a `preview` naming a built-in renderer or a `ui_manifest.json` viewer
(`core/src/filetype/kinds.rs`). The kind's id is `<extension id>:<name>`.
On load the manager rebuilds `FileTypeRegistry::current()` from the built-in
table plus every loaded extension's kinds in load order, and on unload it
rebuilds without them. A kind may refine a file extension the built-in table
maps to its own parent, or claim one the table does not know; a claim that
would redefine a built-in extension is refused at load. Two extensions
claiming one file extension resolve by load order (`load_all` sorts the
directory names), the later claim is dropped from the lookup and reported by
`extensions.list` as a conflict, and its magic patterns still decide a file
whose bytes match only them.

The content identity phase writes the kind and, for an extension kind, its
name onto the content row (`content.kind`, `content.kind_name`, store schema
version 2). Rows identified before an extension existed are named by
extension when the extension loads and when a store opens
(`core/src/ops/indexing/kinds.rs`). A row keeps its name after the extension
unloads; `File.content_kind` is then the parent and `File.content_kind_name`
the name, so a client can fall back until the extension returns.

## Running a job

1. `extensions.run_job` looks the job up in the registry and dispatches a
   `WasmJob` to the library's job manager.
2. `WasmJob::run` loads a checkpoint if one exists, then calls the export on a
   blocking thread with the job context and state JSON written through the
   guest's `wasm_alloc`.
3. The guest's `job_checkpoint` blocks until the state is in the database;
   `job_check_interrupt` reads a flag the async side sets when the core
   interrupts the job. Exit code 0 completes, 1 is interrupted, anything else
   fails.

## Operations

A guest sends `spacedrive_op(name, payload)` and gets back bytes it allocated
through `wasm_alloc`: JSON for a result, or an `OpError` with a stable `code`
(`not_found`, `permission_denied`, `invalid_input`, `not_available`, `failed`)
that the SDK maps onto its own error type.

| Operation | Grant | What it does |
| --- | --- | --- |
| `task.begin`, `task.end` | none | Bracket one `#[task]` attempt. The host writes an attempt line to the job log and refuses a task that outlives the SDK's deadline. |
| `records.get`, `records.query` | `read_records` | A record by uuid, or the records of the library's sources filtered by kind and extension. The optional `glob` on the grant is honored for its trailing extension list only (`*.jpg`, `**/*.{jpg,png}`); the directory part is ignored, and a glob of another shape is refused at load. Tag filters are refused. |
| `records.read` | `read_records` | The bytes of a record, read from its source store's root. |
| `sidecars.exists`, `sidecars.read` | `read_sidecars` | One JSON document under `<library>/sidecars/`, keyed by content uuid and `SidecarKind::Extension { extension_id, kind }`. |
| `sidecars.write` | `write_sidecars` (per kind) | Writes that document. |
| `models.put`, `models.get`, `models.list` | none, models are the extension's own | A row in the extension's store, keyed by the model's uuid or `content:<uuid>` for a content-scoped model. |
| `ai.infer` | `use_models` (per category) | Checks the grant, then answers `not_available`: the core has no inference provider yet. |
| `config.get` | none | `config.json` beside the manifest, or `null`. |

## What the SDK can call today

Everything in the table above, plus `spacedrive_log`, `register_job`,
`register_model`, `spacedrive_random`, `spacedrive_now_ms` and the `job_*`
functions (`job_report_progress`, `job_checkpoint`, `job_check_interrupt`,
`job_add_warning`, `job_increment_items`, `job_increment_bytes`). Tags, custom
fields, dispatching jobs, agents and file-kind or preview registration have no
host side; the SDK returns an error or panics in the guest, and the panic
reaches the host log and fails the job.

## Testing

`core/tests/wasm_extension_test.rs` loads the committed fixtures
`extensions/test-extension/test_extension.wasm` and
`extensions/photos/photos.wasm` against a library with twelve JPEG-magic
files and two text files. The test extension's `catalog` job exercises
records, sidecars, models and tasks; the photos extension's `analyze_photos`
runs end to end and takes the `not_available` path for face detection.
Rebuild a fixture from its crate with `cargo build --release` (each crate's
`.cargo/config.toml` selects wasm32-unknown-unknown) and copy the artifact
next to its manifest. The suite is its own xtask acceptance entry with the
`wasm` feature, because linking wasmer into every test binary is slow.
