## Spacedrive Refactor: Finalization Plan

**Document Version:** 1.0
**Date:** September 13, 2025
**Status:** In Progress

### 1\. Introduction

The foundational refactor of the Spacedrive `Core` engine is a success. We have established a robust, CQRS-based architecture with separate, modular `Action` and `Query` pathways, managed by the `ActionManager` and `QueryManager`. The `Core` now exposes a clean, type-safe API, and the session state has been correctly centralized within its services.

This document outlines the remaining tasks required to fully migrate the codebase to this new architecture, clean up legacy code, and realize the full benefits of the new design.

-----

### 2\. Phase 1: Complete the "Active Library" Context Migration

The primary goal of this phase is to make all operations fully session-aware, removing the need for them to manually handle a `library_id`.

**Objective:** Eliminate the `library_id` field from all `Action` and `Query` structs, and have their handlers source this context directly from the `Core`'s session service.

**Actionable Tasks:**

1.  **Systematically Refactor Remaining `ops` Modules:**

      * Iterate through the following modules and remove the `library_id` field from every `Action` and `Query` struct within them:
          * `ops::location::*` (All location operations)
          * `ops::object::copy`, `ops::object::move`
          * `ops::tag::*` (All tagging operations)
          * `ops::job::*` (All job-related operations)

2.  **Update Handlers to be Session-Aware:**

      * Modify the corresponding `ActionHandler` and `QueryHandler` implementations for the structs above.
      * Inside the `validate` or `execute` methods, retrieve the active library ID by calling a method on the core session service, for example: `core.session().get_active_library_id()?`.

-----

### 3\. Phase 2: Enhance and Finalize Client Applications

With a fully context-aware `Core` API, the clients can be simplified and made more powerful.

**Objective:** Implement the "active library" override mechanism, and reduce boilerplate code in the CLI and GraphQL layers.

**Actionable Tasks:**

1.  **Implement CLI `--library` Flag:**

      * Add a global `--library <LIBRARY_ID>` flag to the CLI using `clap`.
      * In the CLI's main execution logic, check for this flag. If present, call a new method on the `Core`'s session service (e.g., `core.session().set_override_library_id(id)`) before executing the command. This will set the context for the duration of that single operation.

2.  **Reduce Client Boilerplate with `From` Trait:**

      * For each `Action` and `Query` struct, implement the `From<T>` trait, where `T` is the corresponding CLI or GraphQL argument struct.
      * **Example (`core/src/ops/location/add.rs`):**
        ```rust
        // Teach the Action how to be created from CLI args
        impl From<cli::AddLocationArgs> for AddLocationAction {
            fn from(args: cli::AddLocationArgs) -> Self {
                Self { path: args.path, name: args.name }
            }
        }
        ```
      * Refactor the client code to use `.into()` for clean, one-line conversions, eliminating manual field mapping.
        ```rust
        // In the CLI command runner
        core.execute_action(args.into()).await?;
        ```

3.  **Standardize Client Logic with Macros (Optional):**

      * For highly repetitive client patterns (like a simple CLI command that just creates an action, executes it, and prints the result), consider creating `macro_rules!` to generate the boilerplate code automatically.

-----

### 4\. Phase 3: Finalize and Deprecate Legacy Code

The final step is to remove all traces of the old, tightly-coupled architecture.

**Objective:** Ensure the old `DaemonCommand` and its related infrastructure are completely removed from the codebase.

**Actionable Tasks:**

1.  **Delete `DaemonCommand` Enum:**

      * Once all CLI commands have been migrated to the CQRS pattern, the old `DaemonCommand` enum can be safely deleted.

2.  **Clean Up the Daemon:**

      * Remove all logic from the daemon that was responsible for matching on `DaemonCommand` variants. Its role should now be purely to deserialize and route the new `Action`/`Query` structs.

3.  **Code-wide Audit:**

      * Perform a full-text search for `DaemonCommand` and any related types to ensure no remnants are left in the CLI, daemon, or `Core` tests.

By completing these three phases, the refactor will be complete. The result will be a clean, scalable, and highly maintainable architecture that will serve as a robust foundation for the future of Spacedrive.
