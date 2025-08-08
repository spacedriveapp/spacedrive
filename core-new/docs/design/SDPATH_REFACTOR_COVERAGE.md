# Guidance for SdPath Refactoring

This document provides a comprehensive guide for refactoring existing `PathBuf` usages to `SdPath` throughout the Spacedrive codebase. The goal is to fully leverage `SdPath`'s content-addressing and cross-device capabilities, ensuring consistency, resilience, and future extensibility of file operations.

## 1. Core Architectural Principles

The Spacedrive core architecture is structured around three main pillars:

*   **`src/domain` (The Nouns):** Defines the passive, core data structures and types of the system. These are the "things" the system operates on.
*   **`src/operations` (The Verbs):** Contains the active logic and business rules. These modules orchestrate actions using domain entities and infrastructure.
*   **`src/infrastructure` (The Plumbing):** Provides concrete implementations for external interactions (e.g., database access, networking, CLI parsing, filesystem I/O).

### SdPath and PathResolver Placement

*   **`SdPath` (`src/domain/addressing.rs`):** `SdPath` is a fundamental data structure representing a path within the VDFS. It is a "noun" and belongs in the `domain` layer.
*   **`PathResolver` (`src/operations/addressing.rs`):** The `PathResolver` is a service that performs the "resolve" operation on `SdPath`s. It's active logic, a "verb," and thus belongs in the `operations` layer.

This separation ensures high cohesion and a clear, one-way dependency flow (`Operations` depend on `Domain` and `Infrastructure`; `Domain` and `Infrastructure` are independent).

## 2. Understanding SdPath

`SdPath` is an enum designed for universal file addressing:

```rust
pub enum SdPath {
    // A direct pointer to a file at a specific path on a specific device
    Physical {
        device_id: Uuid,
        path: PathBuf,
    },
    // An abstract, location-independent handle that refers to file content
    Content {
        content_id: Uuid,
    },
}
```

### Universal URI Scheme

`SdPath` instances can be represented as standardized URI strings for external interfaces (CLI, API, UI):

*   **Physical Path:** `sd://<device_id>/path/to/file`
*   **Content Path:** `sd://content/<content_id>`

The `SdPath::from_uri(&str)` and `SdPath::to_uri(&self)` methods handle this conversion.

## 3. PathResolver's Role

The `PathResolver` service is responsible for:

*   Taking any `SdPath` (especially `Content` variants) and resolving it to the "best" available `SdPath::Physical` instance.
*   Considering factors like device online status, network latency, and volume performance (cost function).
*   Performing resolution efficiently, ideally in batches (`resolve_batch`).

**Crucial Rule:** `SdPath`s are resolved to `Physical` paths *just before* a file operation is executed (typically within a Job handler).

## 4. Refactoring Guidelines: When to Convert PathBuf to SdPath

When encountering a `PathBuf` usage, determine its purpose:

### Convert to `SdPath` (High Priority)

Replace `PathBuf` with `SdPath` in the following contexts:

*   **Action Definitions (`src/operations/*/action.rs`):** Any action that takes file paths as input or produces them as output.
    *   **Example:** `FileCopyAction`, `FileDeleteAction`, `IndexingAction`, `ThumbnailAction`.
    *   **Rule:** `pub sources: Vec<SdPath>`, `pub destination: SdPath`.
*   **Job Inputs/Outputs (`src/operations/*/job.rs`):** The `Job` struct's fields that represent paths to be operated on.
    *   **Rule:** `pub source_path: SdPath`, `pub target_path: SdPath`.
*   **CLI Command Arguments (`src/infrastructure/cli/daemon/types/commands.rs`):** Command-line arguments that represent file paths should be `String` (URIs) at this layer. The CLI handlers will then parse these `String`s into `SdPath`s.
    *   **Example:** `Copy { sources: Vec<String>, destination: String }`.
*   **API Layer (GraphQL, REST):** Similar to CLI, external API inputs/outputs for paths should be `String` (URIs).
*   **Events (`src/infrastructure/events/mod.rs`):** Events describing file system changes that involve `SdPath` concepts (e.g., `EntryMoved { old_path: SdPath, new_path: SdPath }`).
*   **File Sharing (`src/services/file_sharing.rs`):** Paths involved in cross-device file transfers.

### Keep as `PathBuf` (Lower Priority / Appropriate Usage)

Retain `PathBuf` in the following contexts:

*   **Low-Level Filesystem Interactions (`std::fs`, `std::io`):** When directly interacting with the local operating system's filesystem APIs.
    *   **Example:** Reading file contents, checking `file.exists()`, `file.is_dir()`, creating directories.
    *   **Rule:** These operations should only occur *after* an `SdPath` has been resolved to a `SdPath::Physical` variant, and then `SdPath::as_local_path()` is used to get the `&Path` or `&PathBuf`.
*   **Temporary Files/Directories:** Paths to temporary files or scratch space that are local to the current process or device.
*   **Configuration Paths:** Paths to application data directories, log files, configuration files, or internal database files (e.g., `data_dir`, `log_file`).
*   **Mount Points/Volume Roots:** When referring to the absolute, local filesystem path of a mounted volume or a location's root directory.
*   **Internal Indexer Scans:** The initial discovery phase of the indexer, which directly traverses the local filesystem, will still operate on `PathBuf`. These `PathBuf`s are then converted into `SdPath::Physical` when creating `Entry` records.

## 5. Implementation Details and Best Practices

### 5.1. Action and Job Contracts

*   **Action Definitions:**
    *   Change `PathBuf` fields to `SdPath`.
    *   Update `Into<PathBuf>` generics in builder methods to `Into<SdPath>`.
    *   **Example (`src/operations/files/copy/action.rs`):**
        ```rust
        pub struct FileCopyAction {
            pub sources: Vec<SdPath>,
            pub destination: SdPath,
            pub options: CopyOptions,
        }

        // Builder method example:
        pub fn sources<I, P>(mut self, sources: I) -> Self
        where
            I: IntoIterator<Item = P>,
            P: Into<SdPath>, // Changed from PathBuf
        { /* ... */ }
        ```
*   **Job Execution Flow:**
    *   Any job that operates on files **MUST** resolve its `SdPath` members to `Physical` paths at the beginning of its `run` method.
    *   Use `SdPath::resolve_with(&self, resolver, context)` for single paths or `PathResolver::resolve_batch` for multiple paths.
    *   **Example (`src/operations/files/copy/job.rs`):**
        ```rust
        impl JobHandler for FileCopyJob {
            async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
                // 1. RESOLVE PATHS FIRST
                let resolver = ctx.core_context().path_resolver(); // Assuming resolver is in CoreContext
                let resolved_destination = self.destination.resolve_with(&resolver, ctx.core_context()).await?;
                let resolved_sources_map = resolver.resolve_batch(self.sources.paths.clone(), ctx.core_context()).await;

                // Extract successful resolutions
                let physical_sources: Vec<SdPath> = resolved_sources_map.into_iter()
                    .filter_map(|(_, res)| res.ok())
                    .collect();

                // Ensure destination is physical
                let physical_destination = match resolved_destination {
                    SdPath::Physical { .. } => resolved_destination,
                    _ => return Err(JobError::Validation("Destination must resolve to a physical path".to_string())),
                };

                // ... existing logic now uses physical_sources and physical_destination ...
                // Access underlying PathBuf: physical_path.as_local_path().expect("Must be local physical path")
            }
        }
        ```
*   **Operation Target Validity:**
    *   **Destination/Target:** Operations like copy, move, delete, validate, and index require a physical target. The job's `run` method must ensure the destination `SdPath` is or resolves to a `Physical` variant. An attempt to use a `Content` variant as a final destination is a logical error and should fail.
    *   **Source:** A source can be a `Content` variant, as the resolver will find a physical location for it.

### 5.2. CLI Layer

*   **Command Definitions (`src/infrastructure/cli/daemon/types/commands.rs`):**
    *   Change `PathBuf` fields to `String` (representing URIs).
    *   **Example:**
        ```rust
        pub enum DaemonCommand {
            Copy {
                sources: Vec<String>, // AFTER (as URIs)
                destination: String,  // AFTER (as a URI)
                // ... options
            },
        }
        ```
*   **Command Handlers (`src/infrastructure/cli/daemon/handlers/`):**
    *   Responsible for parsing these `String` URIs into `SdPath` enums *before* creating and dispatching an `Action`.
    *   Handle `SdPathParseError` gracefully.

### 5.3. Copy Strategy and Routing

*   **`src/operations/files/copy/routing.rs`:**
    *   The `CopyStrategyRouter::select_strategy` function must be refactored.
    *   **Rule:** It should receive *already resolved* `SdPath::Physical` instances for source and destination.
    *   Compare the `device_id` of the two `Physical` paths.
    *   If `device_id`s are the same, use `VolumeManager` to check if they are on the same volume and select `LocalMoveStrategy` or `LocalStreamCopyStrategy`.
    *   If `device_id`s differ, select `RemoteTransferStrategy`.
*   **`src/operations/files/copy/strategy.rs`:**
    *   Strategy implementations (`LocalMoveStrategy`, `LocalStreamCopyStrategy`, `RemoteTransferStrategy`) should only accept `SdPath::Physical` variants.
    *   Their internal logic will then use `SdPath::as_local_path()` to get the underlying `PathBuf` for `std::fs` operations.

## 6. Common Pitfalls and Considerations

*   **N+1 Query Problem:** Always prioritize batch resolution (`PathResolver::resolve_batch`) when dealing with multiple paths to minimize database and network round-trips.
*   **Error Handling:** Ensure `PathResolutionError` and `SdPathParseError` are propagated and handled appropriately.
*   **Validation Shift:** Remember that filesystem-level validations (e.g., `path.exists()`) should generally occur *after* path resolution within the job execution, not during action creation.
*   **Testing:** Update unit and integration tests to:
    *   Construct `SdPath` instances using `SdPath::physical`, `SdPath::content`, `SdPath::local`, or `SdPath::from_uri`.
    *   Assert on the correct `SdPath` variant and its internal fields.
    *   Mock or simulate `PathResolver` behavior for unit tests where appropriate.
*   **Performance:** The cost function within `PathResolver` is critical for performance. Ensure it accurately reflects real-world latency and bandwidth.
*   **`SdPathBatch`:** This helper struct can be useful for grouping `SdPath`s, especially when passing them to `PathResolver::resolve_batch`.

By following these guidelines, the codebase will evolve to fully embrace the power and flexibility of `SdPath`, making Spacedrive's file management truly content-aware and resilient.
