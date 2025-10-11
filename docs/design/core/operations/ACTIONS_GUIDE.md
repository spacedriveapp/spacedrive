<!--CREATED: 2025-10-11-->
## Spacedrive Actions: Architecture and Authoring Guide

This document explains the current Action System in `sd-core`, how actions are discovered and dispatched, how inputs/outputs are shaped, how domain paths (`SdPath`, `SdPathBatch`) are used, and how to add new actions consistently.

### Scope at a Glance

- Core files:
  - `core/src/infra/action/mod.rs` — traits for `CoreAction` and `LibraryAction`
  - `core/src/ops/registry.rs` — action/query registry and registration macros
  - `core/src/infra/action/manager.rs` — `ActionManager` that validates, audits and executes actions
  - Domain paths: `core/src/domain/addressing.rs` (`SdPath`, `SdPathBatch`)
- Job system integration:
  - Actions frequently dispatch Jobs and return a `JobHandle`
  - Job progress is emitted via `EventBus` (see `core/src/infra/event/mod.rs`)

### Action Traits

There are two flavors of actions:

- `CoreAction` — operates without a specific library context (e.g., creating/deleting a library):

  - Associated types: `type Input`, `type Output`
  - `from_input(input) -> Self` — build action from wire input
  - `async fn execute(self, context: Arc<CoreContext>) -> Result<Output, ActionError>`
  - `fn action_kind(&self) -> &'static str`
  - Optional `async fn validate(&self, context)`

- `LibraryAction` — operates within a specific library (files, locations, indexing, volumes):
  - Associated types: `type Input`, `type Output`
  - `from_input(input) -> Self`
  - `async fn execute(self, library: Arc<Library>, context: Arc<CoreContext>) -> Result<Output, ActionError>`
  - `fn action_kind(&self) -> &'static str`
  - Optional `async fn validate(&self, &Arc<Library>, context)`

Both traits are implemented directly on the action struct. The manager handles orchestration (validation, audit log, execution).

### Registry & Wire Methods

`core/src/ops/registry.rs` provides macros that register actions and queries using the `inventory` crate.

- Library actions use:

  ```rust
  crate::register_library_action!(MyAction, "group.operation");
  ```

  This generates:

  - A wire method on the input type: `action:group.operation.input.v1`
  - An inventory `ActionEntry` bound to `handle_library_action::<MyAction>`

- Queries use `register_query!(QueryType, "group.name");`

Naming convention for wire methods:

- `action:<name>.input.v1` for action inputs
- `query:<name>.v1` for queries

The daemon/API can route calls by these method strings to decode inputs and trigger the right handler.

### ActionManager Flow (Library Actions)

`ActionManager::dispatch_library(library_id, action)`:

1. Loads and validates the library (ensures it exists)
2. Calls `action.validate(&library, context)` (optional)
3. Creates an audit log entry
4. Executes `action.execute(library, context)`
5. Finalizes the audit log with success/failure

For `CoreAction`, `dispatch_core(action)` follows a similar path without a library.

### Domain Paths: `SdPath` and `SdPathBatch`

Actions operate on Spacedrive domain paths, not raw filesystem strings:

- `SdPath` — can be a `Physical { device_id, path }` or `Content { content_id }`. `SdPath::local(path)` creates a physical path on the current device.
- `SdPathBatch` — a simple wrapper: `struct SdPathBatch { pub paths: Vec<SdPath> }`

Guidelines:

- Prefer `SdPath` in action inputs/outputs rather than `PathBuf`
- For multi-path inputs, use `SdPathBatch`
- When you need a local path at execution time, use helpers like `as_local_path()`

Example (from Files Copy):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCopyAction {
    pub sources: SdPathBatch,
    pub destination: SdPath,
    pub options: CopyOptions,
}

impl LibraryAction for FileCopyAction { /* ... */ }
```

### Inputs and Builders

Actions often define an explicit `Input` type for the wire contract and a small builder or convenience API to create well-formed actions from CLI/REST/GraphQL translators. Example: `FileCopyInput` maps CLI flags into a `CopyOptions` plus `SdPath`/`SdPathBatch` and conversions happen in `from_input`.

Validation layers:

- Syntactic/cheap validation in `Input::validate()` (returning a vector of errors)
- Action-level `validate(...)` invoked by the manager before `execute`

### Job Dispatch & Outputs

For long-running operations (copy, delete, indexing), actions typically create and dispatch a job via the library job manager, returning a `JobHandle` as the action output. Example:

```rust
let job = FileCopyJob::new(self.sources, self.destination).with_options(self.options);
let job_handle = library.jobs().dispatch(job).await?;
Ok(job_handle)
```

Progress and completion events are emitted on the `EventBus` (`Event::JobProgress`, `Event::JobCompleted`, etc.).

### Current Registered Operations

Discovered via registry:

- Library actions (registered):

  - `files.copy`
  - `files.delete`
  - `files.duplicate_detection`
  - `files.validation`
  - `indexing.start`

- Queries (registered):
  - `core.status`
  - `libraries.list`

Implemented but not yet registered (present `impl LibraryAction` without `register_library_action!`):

- `locations.add`, `locations.remove`, `locations.rescan`
- `libraries.export`, `libraries.rename`
- `volumes.track`, `volumes.untrack`, `volumes.speed_test`
- `media.thumbnail`

Implemented `CoreAction` (not yet registered via a core registration macro):

- `library.create`, `library.delete`

> Note: Core action registration would use a `register_core_action!` macro similar to library actions. The registry contains such a macro, but it is not yet invoked for these actions.

### Authoring a New Library Action (Checklist)

1. Define your wire `Input` type:

   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct MyOpInput { /* fields using SdPath / SdPathBatch / options */ }
   ```

2. Define your action struct and implement `LibraryAction`:

   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct MyOpAction { input: MyOpInput }

   impl LibraryAction for MyOpAction {
       type Input = MyOpInput;
       type Output = /* domain type or JobHandle */;

       fn from_input(input: MyOpInput) -> Result<Self, String> { Ok(Self { input }) }

       async fn validate(&self, _lib: &Arc<Library>, _ctx: Arc<CoreContext>) -> Result<(), ActionError> {
           // cheap checks; return ActionError::Validation { field, message } on invalid
           Ok(())
       }

       async fn execute(self, library: Arc<Library>, ctx: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
           // do the work or dispatch a job
           Ok(/* output */)
       }

       fn action_kind(&self) -> &'static str { "group.operation" }
   }
   ```

3. Register the action:

   ```rust
   crate::register_library_action!(MyOpAction, "group.operation");
   // Wire method will be: action:group.operation.input.v1
   ```

4. Ensure inputs use `SdPath`/`SdPathBatch` appropriately. For multiple paths:

   ```rust
   let batch = SdPathBatch::new(vec![SdPath::local("/path/a"), SdPath::local("/path/b")]);
   ```

5. Prefer returning native domain outputs or `JobHandle` for long-running tasks.

6. Emit appropriate `EventBus` events from jobs for progress UX.

### Conventions & Tips

- `action_kind()` should match your domain naming (`"files.copy"`, `"volumes.track"`, etc.)
- Keep builders thin; ensure `from_input()` is the canonical wire adapter
- Put expensive I/O in the `execute` or in jobs, not in validation
- Use `ActionError::Validation { field, message }` for user-facing errors
- When interacting with the filesystem, always resolve/check local paths via `SdPath::as_local_path()`

### Minimal Example

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExampleInput { pub targets: SdPathBatch }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExampleAction { input: ExampleInput }

impl LibraryAction for ExampleAction {
    type Input = ExampleInput;
    type Output = JobHandle;

    fn from_input(input: ExampleInput) -> Result<Self, String> { Ok(Self { input }) }

    async fn validate(&self, _lib: &Arc<Library>, _ctx: Arc<CoreContext>) -> Result<(), ActionError> {
        if self.input.targets.paths.is_empty() {
            return Err(ActionError::Validation { field: "targets".into(), message: "At least one target required".into() });
        }
        Ok(())
    }

    async fn execute(self, library: Arc<Library>, _ctx: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
        let job = /* build job from self.input */;
        let handle = library.jobs().dispatch(job).await?;
        Ok(handle)
    }

    fn action_kind(&self) -> &'static str { "example.run" }
}

crate::register_library_action!(ExampleAction, "example.run");
```

---

This guide reflects the current state of the action system. As we register additional actions (locations, volumes, media thumbnailing, library core actions), follow the same patterns for naming, inputs, validation, and registration.
