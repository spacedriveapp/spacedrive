Of course. Based on a thorough review of the Spacedrive whitepaper and the provided Rust codebase, here is a detailed design document for the Simulation Engine. This design aims for a clean, non-disruptive integration that leverages the existing architectural patterns.

---

# Design Document: The Spacedrive Simulation Engine

## 1\. Executive Summary

The Spacedrive whitepaper outlines a key innovation: a **Transactional Action System with Pre-visualization**. This system allows any file operation to be simulated in a "dry run" mode, providing users with a detailed preview of the outcome—including space savings, conflicts, and time estimates—before committing to the action.

This document details the design and integration of the **Simulation Engine**, the core component responsible for generating these previews. The engine will be integrated directly into the existing `Action` infrastructure, operating on the VDFS index as a read-only source of truth. It will produce a structured `ActionPlan` that can be consumed by any client (GUI, CLI, TUI) to render the pre-visualization described in the whitepaper.

**Core Principles:**

- **Index-First:** The simulation relies exclusively on the library's database index and volume metadata, never touching the actual files on disk.
- **Read-Only:** The simulation process is strictly a read-only operation, guaranteeing it has no side effects.
- **Handler-Based Logic:** The simulation logic for each action type (e.g., `FileCopy`, `FileDelete`) will be encapsulated within its corresponding `ActionHandler`, ensuring modularity and extensibility.

## 2\. Goals and Core Concepts

The primary goal is to build an engine that can take any `Action` and produce a detailed, verifiable plan of execution.

- **Action Plan:** The structured output of the simulation. It contains a summary, a list of steps, estimated metrics, and any potential conflicts.
- **Simulation vs. Execution:** Simulation is the predictive, read-only process that generates an `ActionPlan`. Execution is the "commit" phase where the `ActionManager` dispatches a job to perform the actual file operations.
- **Path Resolution:** A precursor to simulation. Given a content-aware or physical `SdPath`, this step determines the optimal physical source path(s) for the operation based on device availability, network latency, and storage tier.

The engine must achieve the following goals outlined in the whitepaper:

1.  **Conflict Detection:** Proactively identify issues like insufficient storage, permission errors, and path conflicts.
2.  **Resource Prediction:** Provide accurate estimates for storage changes, network usage, and completion time.
3.  **State Pre-visualization:** Clearly articulate the final state of the filesystem after the action completes.
4.  **Safety and User Control:** Empower the user to make an informed decision before any irreversible changes are made.

## 3\. Architectural Integration

The Simulation Engine will be integrated into the existing `Action` system with minimal disruption. The current action lifecycle is `validate -> execute`. We will introduce `simulate` as a distinct, preliminary step.

### 3.1. New Action Lifecycle

1.  **Client (UI/CLI):** Creates an `Action` struct (e.g., `FileCopyAction`).
2.  **Client:** Calls a new `action_manager.simulate(action)` method.
3.  **Simulation Engine:**
    - Resolves all source `SdPath`s to their optimal physical paths.
    - Invokes the `simulate` method on the appropriate `ActionHandler`.
    - The handler queries the VDFS index and volume metadata via the `CoreContext`.
    - The handler returns a structured `ActionPlan`.
4.  **Client:** Renders the `ActionPlan` for user review (as seen in Figure 8 of the whitepaper).
5.  **User:** Approves the plan.
6.  **Client:** Calls the existing `action_manager.dispatch(action)` method to commit the operation.
7.  **ActionManager:** Dispatches the action to the Durable Job System for execution.

### 3.2. Key Component Modifications

#### `ActionHandler` Trait (`src/infrastructure/actions/handler.rs`)

The `ActionHandler` trait is the ideal place to encapsulate simulation logic. A new `simulate` method will be added.

```rust
#[async_trait]
pub trait ActionHandler: Send + Sync {
    /// Execute the action and return output
    async fn execute(...) -> ActionResult<ActionOutput>;

    /// Validate the action before execution (optional)
    async fn validate(...) -> ActionResult<()>;

    /// **[NEW]** Simulate the action and return a detailed plan
    async fn simulate(
        &self,
        context: Arc<CoreContext>,
        action: &Action,
    ) -> ActionResult<ActionPlan>;

    /// Check if this handler can handle the given action
    fn can_handle(&self, action: &Action) -> bool;

    /// Get the action kinds this handler supports
    fn supported_actions() -> &'static [&'static str];
}
```

#### `ActionManager` (`src/infrastructure/actions/manager.rs`)

A new public `simulate` method will be the primary entry point for the engine.

```rust
impl ActionManager {
    /// **[NEW]** Simulate an action to generate a preview.
    pub async fn simulate(
        &self,
        action: Action,
    ) -> ActionResult<ActionPlan> {
        // 1. Find the correct handler in the registry
        let handler = REGISTRY
            .get(action.kind())
            .ok_or_else(|| ActionError::ActionNotRegistered(action.kind().to_string()))?;

        // 2. Perform initial validation
        handler.validate(self.context.clone(), &action).await?;

        // 3. Execute the simulation
        handler.simulate(self.context.clone(), &action).await
    }

    /// Dispatch an action for execution (existing method)
    pub async fn dispatch(...) -> ActionResult<ActionOutput> {
        // ... existing implementation ...
    }
}
```

#### `SdPath` & `SdPathBatch` (`src/shared/types.rs`)

As requested, these structs will gain a `resolve` method to find the optimal physical path. This will be used by the simulation engine.

```rust
// In SdPath struct
impl SdPath {
    /// Resolves the SdPath to the optimal physical path.
    /// For content-aware paths, this performs a lookup.
    /// For physical paths, it verifies availability.
    pub async fn resolve(
        &self,
        context: &CoreContext
    ) -> Result<PhysicalPath, PathResolutionError> {
        // ... logic using VolumeManager and NetworkService ...
    }
}

// In SdPathBatch struct
impl SdPathBatch {
    /// Resolves all paths in the batch.
    pub async fn resolve_all(
        &self,
        context: &CoreContext
    ) -> Result<Vec<PhysicalPath>, PathResolutionError> {
        // ... parallel resolution logic ...
    }
}
```

## 4\. New Data Structures

To support the simulation, several new data structures are required to model the `ActionPlan`.

#### `ActionPlan`

This is the primary output of the simulation. It contains all information needed for the UI to render a preview.

```rust
/// A detailed, structured plan of an action's effects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionPlan {
    /// A high-level, human-readable summary.
    /// e.g., "Move 1,224 unique files (8.1 GB) to Home-NAS"
    pub summary: String,

    /// A step-by-step breakdown of the physical operations.
    pub steps: Vec<ExecutionStep>,

    /// Estimated metrics for the operation.
    pub metrics: EstimatedMetrics,

    /// A list of potential conflicts or issues detected.
    pub warnings: Vec<ConflictWarning>,

    /// A simple flag indicating if the operation is considered safe to proceed.
    pub is_safe: bool,
}
```

#### `ExecutionStep`

An enum representing a single, atomic operation within the plan.

```rust
/// A single physical step in the execution of an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStep {
    Read {
        source: SdPath,
        size: u64,
    },
    Transfer {
        source_device: Uuid,
        destination_device: Uuid,
        size: u64,
    },
    Write {
        destination: SdPath,
        size: u64,
    },
    Delete {
        target: SdPath,
        is_permanent: bool,
    },
    Skip {
        target: SdPath,
        reason: String, // e.g., "Duplicate content exists at destination"
    },
}
```

#### `EstimatedMetrics`

A struct to hold all predicted metrics for the operation.

```rust
/// Predicted metrics for an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EstimatedMetrics {
    pub files_to_process: u64,
    pub total_size_bytes: u64,
    pub duplicate_files_skipped: u64,
    pub duplicate_size_bytes_saved: u64,
    pub required_space_bytes: u64,
    pub estimated_duration_secs: u64,
    pub estimated_network_usage_bytes: u64,
}
```

#### `ConflictWarning`

An enum representing potential issues the user should be aware of.

```rust
/// A potential conflict or issue detected during simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictWarning {
    InsufficientSpace {
        destination: SdPath,
        required: u64,
        available: u64,
    },
    PermissionDenied {
        path: SdPath,
        operation: String, // "read" or "write"
    },
    DestinationExists {
        path: SdPath,
    },
    SourceIsOffline {
        device_id: Uuid,
    },
    PerformanceMismatch {
        message: String, // e.g., "This targets a 'hot' location on a slow archive drive."
    },
}
```

## 5\. The Simulation Process in Detail: A `FileCopy` Example

Let's trace a `FileCopyAction` for a cross-device move, as described in the whitepaper.

1.  **User Intent:** The user initiates a move of `~/Photos/2024` from their MacBook to `/backups/photos` on their `Home-NAS`.

2.  **Action Creation:** The client creates an `Action::FileCopy` with `delete_after_copy: true`.

3.  **Simulation Request:** The client calls `action_manager.simulate(action)`.

4.  **Handler Invocation:** The `ActionManager` finds the `FileCopyHandler` and calls its `simulate` method.

5.  **Simulation Logic within `FileCopyHandler::simulate`:**
    a. **Path Resolution:** The handler calls `sources.resolve_all(&context)`. This resolves the `~/Photos/2024` directory into a list of all physical file paths within it by querying the library's index for that location.
    b. **Data Gathering:** The handler uses the `CoreContext` to gather necessary information: \* From `VolumeManager`: Gets the `PhysicalClass` (e.g., `Hot` for the MacBook's SSD, `Cold` for the NAS HDD) and available space on both source and destination volumes. \* From `NetworkingService`: Gets the current bandwidth and latency between the MacBook and the NAS. \* From the Library DB: For each source file, it looks up the `ContentId`. For each `ContentId`, it queries if an entry already exists at the destination.
    c. **Build Execution Steps & Metrics:** \* It iterates through the resolved source paths. \* For a file that is a duplicate at the destination, it creates a `ExecutionStep::Skip` and adds its size to `duplicate_size_bytes_saved`. \* For a unique file, it creates `ExecutionStep::Read`, `ExecutionStep::Transfer`, and `ExecutionStep::Write` steps. It also adds a `ExecutionStep::Delete` because this is a move operation. \* It aggregates the total size of files to be processed into `total_size_bytes`.
    d. **Conflict & Performance Checks:** \* It compares `total_size_bytes` with the available space on the NAS. If insufficient, it adds a `ConflictWarning::InsufficientSpace`. \* It compares the `LogicalClass` of the source/destination locations with the `PhysicalClass` of the volumes. If there's a mismatch (e.g., a "Hot" location on a "Cold" drive), it adds a `ConflictWarning::PerformanceMismatch`.
    e. **Estimate Duration:** It uses the total size, volume performance metrics, and network metrics to calculate `estimated_duration_secs`.
    f. **Assemble `ActionPlan`:** It packages all the generated steps, metrics, and warnings into a final `ActionPlan` object.

6.  **Return to Client:** The `ActionPlan` is returned to the client, which can now render a detailed, interactive preview for the user, fulfilling the vision of the whitepaper.

## 6\. Implementation Snippets

#### `src/infrastructure/actions/handler.rs` (Modified `ActionHandler` trait)

```rust
// ...
use crate::infrastructure::actions::plan::ActionPlan; // New module

#[async_trait]
pub trait ActionHandler: Send + Sync {
    async fn execute(
        &self,
        context: Arc<CoreContext>,
        action: Action,
    ) -> ActionResult<ActionOutput>;

    async fn validate(
        &self,
        _context: Arc<CoreContext>,
        _action: &Action,
    ) -> ActionResult<()> {
        Ok(())
    }

    /// **[NEW]**
    async fn simulate(
        &self,
        context: Arc<CoreContext>,
        action: &Action,
    ) -> ActionResult<ActionPlan>;

    fn can_handle(&self, action: &Action) -> bool;

    fn supported_actions() -> &'static [&'static str] where Self: Sized;
}
```

#### `src/operations/files/copy/handler.rs` (Example implementation)

```rust
// ...
use crate::infrastructure::actions::plan::{ActionPlan, ExecutionStep, EstimatedMetrics, ConflictWarning};

#[async_trait]
impl ActionHandler for FileCopyHandler {
    // ... existing execute and validate methods ...

    async fn simulate(
        &self,
        context: Arc<CoreContext>,
        action: &Action,
    ) -> ActionResult<ActionPlan> {
        if let Action::FileCopy { action, .. } = action {
            let mut steps = Vec::new();
            let mut warnings = Vec::new();
            let mut metrics = EstimatedMetrics::default();

            // 1. Resolve source paths from the index
            // ... logic to get all individual file SdPaths from source directories ...

            // 2. Gather context
            let destination_volume = context.volume_manager
                .volume_for_path(&action.destination)
                .await;

            // 3. Process each file
            // for source_path in resolved_sources {
                // a. Check for duplicates at destination via ContentId
                // b. Check for space on destination_volume
                // c. Add ExecutionSteps (Read, Transfer, Write, Delete/Skip)
                // d. Aggregate metrics
            // }

            // 4. Finalize plan
            Ok(ActionPlan {
                summary: format!("Simulated moving {} files", metrics.files_to_process),
                steps,
                metrics,
                warnings,
                is_safe: true, // Set based on warnings
            })
        } else {
            Err(ActionError::InvalidActionType)
        }
    }
}
```

## 7\. Future Considerations

- **UI Integration:** The `ActionPlan` struct is designed to be easily serialized to JSON, making it straightforward for any frontend to consume and render.
- **Complex Workflows:** AI-generated actions that involve multiple steps can be represented as a `Vec<ActionPlan>`, allowing the user to review a complete, multi-stage workflow before committing.
- **Undo/Redo:** A committed and executed `ActionPlan` can be stored in the audit log. This provides a perfect artifact for generating a compensatory "undo" action, paving the way for intelligent undo capabilities as described in the whitepaper.
