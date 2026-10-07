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
  module. `spacedrive_log` and `register_job` serve `plugin_init`; the `job_*`
  functions report into the job that is running through a `JobBridge`.
- `job_registry.rs`: the jobs extensions registered, keyed
  `<extension id>:<job name>`.
- `wasm_job.rs`: the one core job type that runs every extension job. It
  holds the guest's state as a JSON string and persists it through the normal
  checkpoint table, so a kill or a pause resumes from the guest's last
  checkpoint.
- `permissions.rs`, `types.rs`: the manifest and the capability model for
  `spacedrive_call`, which routes a Wire method through the operation
  registry. The SDK does not import it yet.

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

## What the SDK can call today

`spacedrive_log`, `register_job`, `job_report_progress`, `job_checkpoint`,
`job_check_interrupt`, `job_add_warning`, `job_increment_items`,
`job_increment_bytes`. Everything else in `crates/sdk` (VDFS entries, sidecars,
AI, models, tags, tasks, agents, entropy, clock) has no host function and
panics in the guest; the panic reaches the host log and the job fails.

## Testing

`core/tests/wasm_extension_test.rs` loads the committed fixtures
`extensions/test-extension/test_extension.wasm` and
`extensions/photos/photos.wasm`. Rebuild a fixture from its crate with
`cargo build --release` (each crate's `.cargo/config.toml` selects
wasm32-unknown-unknown) and copy the artifact next to its manifest.
