### Operations Initialization API (op!)

This document defines a simple, uniform API for declaring and registering all operations (library actions, core actions, and queries) in Core. The goals are:

- Make operations easy to add, understand, and maintain
- Keep inputs pure (no session, no library_id)
- Keep actions free of transport concerns (no library_id in action structs)
- Provide consistent method naming and automatic inventory registration
- Reduce boilerplate and promote repeatable patterns

---

### Core Concepts

- **Input**: External API payload (CLI/GraphQL). Contains only operation-specific fields.
- **Action**: Executable business logic. Receives `Arc<Library>` (for library actions) or only `CoreContext` (for core actions) during execution.
- **Query**: Read-only operation returning data.
- **Method**: Opaque string used for routing (e.g., `action:files.copy.input.v1`, `query:core.status.v1`).
- **Registry**: Inventory-backed lookup that dispatches by method.

Inputs are decoded, converted to actions (for actions), and executed. For library actions, the `library_id` is resolved by the dispatcher and never appears in inputs or actions.

---

### The `op!` Macro (Proposed)

One macro, three variants. Uniform API for every operation type.

- Library actions:

```rust
op!(library_action InputType => ActionType, "files.copy", via = BuilderType);
// or, if no builder is needed:
op!(library_action InputType => ActionType, "files.validation");
```

- Core actions:

```rust
op!(core_action InputType => ActionType, "libraries.create", via = BuilderType);
// or
op!(core_action InputType => ActionType, "libraries.delete");
```

- Queries:

```rust
op!(query QueryType, "core.status");
```

#### What `op!` does

- Generates the method string automatically:
  - Library/Core actions → `action:{domain}.{op}.input.v1`
  - Queries → `query:{domain}.{op}.v1`
- Implements `Wire` for the type with that method
- Implements the appropriate build/dispatch glue and registers with `inventory`
- For actions, wires Input → Action using one of:
  - `TryFrom<Input> for Action` (preferred), or
  - `via = BuilderType` (calls `BuilderType::from_input(input).build()`) if provided

This keeps every operation declaration small and consistent.

---

### Conversion: Input → Action

Use `TryFrom<Input> for Action` as the single source of truth for how inputs become actions. For complex operations that already have builders, the `TryFrom` impl can delegate to the builder.

Example (File Copy):

```rust
impl TryFrom<FileCopyInput> for FileCopyAction {
    type Error = String;
    fn try_from(input: FileCopyInput) -> Result<Self, Self::Error> {
        use crate::infra::action::builder::ActionBuilder;
        FileCopyActionBuilder::from_input(input).build().map_err(|e| e.to_string())
    }
}

op!(library_action FileCopyInput => FileCopyAction, "files.copy");
```

Example (Validation – direct construction):

```rust
impl TryFrom<FileValidationInput> for ValidationAction {
    type Error = String;
    fn try_from(input: FileValidationInput) -> Result<Self, Self::Error> {
        Ok(ValidationAction::new(input.paths, input.verify_checksums, input.deep_scan))
    }
}

op!(library_action FileValidationInput => ValidationAction, "files.validation");
```

---

### Method Naming & Versioning

- Actions: `action:{domain}.{operation}.input.v{version}`
- Queries: `query:{domain}.{operation}.v{version}`
- Default version: `v1`. Bump when the wire contract changes.
- Keep `{domain}.{operation}` short, stable, and human-readable.

Optional helpers:

- `action_method!("files.copy")` → `"action:files.copy.input.v1"`
- `query_method!("core.status")` → `"query:core.status.v1"`

`op!` can call these internally so call sites only specify `"files.copy"` or `"core.status"`.

---

### Dispatch Flow (Library Action)

1. Client sends `Input` with `method`
2. Core registry decodes and builds `Action` (pure conversion)
3. Registry resolves `library_id` from session
4. `ActionManager::dispatch_library(library_id, action)`
5. Manager fetches `Arc<Library>`, validates, creates audit entry, then calls `action.execute(library, context)`

Core actions are the same minus step 3.

---

### Implementation Checklist

- Traits/Manager (done):
  - `LibraryAction` has no `library_id()` requirement
  - `ActionManager::dispatch_library(library_id, action)` resolves library and logs
- Registry (done):
  - `BuildLibraryActionInput::build(self) -> Action` (pure)
  - Handler resolves `library_id` from session once
- Inputs/Actions:
  - Inputs are pure (no session/library_id)
  - Actions do not store `library_id`
  - Add `TryFrom<Input> for Action` (delegate to builder when needed)
- Macro:
  - Provide `op!` (three variants) and method helpers (optional)

---

### Migration Plan

1. Introduce `op!` and helpers in `ops::registry`
2. Convert existing operations:
   - Files: copy, delete, validation, duplicate_detection (copy: via builder; others: direct TryFrom)
   - Indexing: implement TryFrom or use builder; remove library_id from action if present
   - Libraries/Core ops: create/rename/delete via `op!(core_action …)`
   - Queries: swap to `op!(query …)` where appropriate
3. Delete old per-op registration boilerplate
4. Run registry tests to verify:
   - All required methods registered
   - Naming convention checks pass
   - No duplicates across queries/actions

---

### Examples (End-to-End)

- File Copy:

```rust
impl TryFrom<FileCopyInput> for FileCopyAction { /* delegate to builder */ }
op!(library_action FileCopyInput => FileCopyAction, "files.copy");
```

- Validation:

```rust
impl TryFrom<FileValidationInput> for ValidationAction { /* construct directly */ }
op!(library_action FileValidationInput => ValidationAction, "files.validation");
```

- Library Create (Core Action):

```rust
impl TryFrom<LibraryCreateInput> for LibraryCreateAction { /* construct directly */ }
op!(core_action LibraryCreateInput => LibraryCreateAction, "libraries.create");
```

- Core Status (Query):

```rust
op!(query CoreStatusQuery, "core.status");
```

---

### Testing & Tooling

- Use existing registry tests:

  - `test_method_naming_convention`
  - `test_has_registered_operations`
  - `test_no_duplicate_methods`
  - `list_registered_operations()` to debug

- Add unit tests for `TryFrom<Input> for Action` where logic is non-trivial (builder path, validation errors).

---

### Do & Don’t

- **Do** keep inputs pure and actions context-free
- **Do** resolve `library_id` once at dispatch time
- **Do** prefer `TryFrom<Input> for Action` and reuse builders when present
- **Don’t** put `library_id` into inputs or actions
- **Don’t** pass `SessionState` into `build()`
- **Don’t** hardcode method strings inconsistently—use `op!`

---

### Future Extensions

- Derive `#[derive(LibraryOpInput("files.copy"))]` to generate `TryFrom`, `Wire`, and registration automatically for simple ops
- Add lint to enforce method naming and versioning conventions
- Method helpers `action_method!`/`query_method!` to centralize formatting
