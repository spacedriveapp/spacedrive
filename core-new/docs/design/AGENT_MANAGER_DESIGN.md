# Agent Manager Design

## Overview

The **Agent Manager** is a core component of Spacedrive's AI-native architecture. It is responsible for creating, managing, and orchestrating AI agents that can perform intelligent file management tasks. These agents translate natural language commands into verifiable operations and can proactively suggest optimizations based on user behavior.

Agents execute tasks by generating commands for Spacedrive's native **Command Line Interface (CLI)**, which is bundled with the application and communicates with the running core via a secure IPC channel. This ensures all AI-driven operations are subject to the same safety and permission constraints as those performed by a human user.

## Core Components

- **Agent Manager:** The central singleton responsible for the agent lifecycle. It initializes agents, assigns them tasks, and manages their access to system resources.
- **Agent Trait:** A common Rust `trait` that defines the capabilities of any agent, such as receiving a task, processing it, and reporting the outcome.
- **LLM Provider Interface:** A pluggable interface for communicating with different Large Language Models, supporting both local providers (like Ollama) and cloud-based services.
- **Execution Coordinator:** This component is responsible for spawning the `sd` CLI sidecar process with the commands formulated by an agent. It manages the process lifecycle and captures its output (stdout/stderr) for the agent to observe and parse.

## Agentic Loop (Observe-Orient-Decide-Act)

Each agent operates on a continuous loop, enabling it to handle complex, multi-step tasks.

1.  **Observe:** The agent analyzes the current state. This includes the initial user query, the file system state (via the VDFS index), and the results of previous actions. A key part of observation is **parsing the `stdout` from previously executed CLI commands** and, if necessary, querying the audit log via `sd log` to confirm the outcome.
2.  **Orient:** The agent uses an LLM to interpret the observed state and understand the context of its task.
3.  **Decide:** The agent formulates a plan, which may consist of one or more steps. For example, to "archive old projects," the plan would be to first _find_ the projects and then _move_ them.
4.  **Act:** The agent executes the next step in its plan. In this architecture, "acting" means **generating a specific, validated command for the `sd` CLI and instructing the Execution Coordinator to run it.**

## LLM Integration and Tooling

### CLI as the Primary Tool Interface

The integration between the AI layer and the Spacedrive core is intentionally decoupled for security and modularity. Instead of direct function calls, agents use the `sd` CLI as their sole tool.

- **Bundling and Availability:** The `sd` CLI is packaged as a **Tauri sidecar** and bundled with every Spacedrive installation. This ensures that the Rust core can always locate and execute a compatible version of the CLI.
- **IPC Communication:** The architecture uses a daemon-client model. The main Spacedrive core runs a **JSON-RPC server** on a local socket. When the Execution Coordinator spawns the `sd` CLI process, the CLI detects the running daemon, connects as a client, and transmits its command for execution.
- **Schema-Aware Prompting:** The Agent Manager automatically provides the LLM with the CLI's schema, help text, and usage examples within the context prompt. This **few-shot learning** approach enables the LLM to correctly format commands for the available tools without requiring specialized fine-tuning.
- **Safety and Sandboxing:** This model creates a robust security boundary. The LLM's capabilities are strictly limited to what the CLI exposes, which in turn is governed by the safe, verifiable, and auditable **Transactional Action System**. The AI has no direct access to internal state or the database.

## Key Data Structures

```rust
/// Represents a task assigned to an agent by the manager.
pub struct AgentTask {
    pub id: Uuid,
    pub user_prompt: String, // e.g., "organize my tax documents"
    pub status: TaskStatus,
    pub commands_executed: Vec<String>, // Log of CLI commands run
    pub results: String, // Final summary for the user
}

/// The current status of an agent's task.
pub enum TaskStatus {
    Pending,
    Running,
    AwaitingConfirmation(ActionPreview), // Paused for user approval
    Completed,
    Failed(String),
}
```

## Security and Sandboxing

Security is a primary design consideration. By forcing all AI-driven operations through the CLI, we ensure the LLM operates within the same permission and validation boundaries as a human user. It has no direct access to internal state or database connections, significantly reducing the attack surface. Every action taken by an agent is auditable through the standard system log, just like any other action.

### Agent Long-Term Memory

To enable learning and context retention across tasks, each agent instance is provided with its own private, persistent memory space managed directly by Spacedrive.

- **Memory as a Virtual Directory:** When an agent is first initialized, the Agent Manager creates a dedicated, hidden directory for it within the `.sdlibrary` package (e.g., `.sdlibrary/agents/<agent-id>/`). This directory is the agent's exclusive long-term memory store.

- **Structured Memory Files:** Within this directory, an agent maintains a set of files to store different types of information:

  - `scratchpad.md`: For short-term thoughts and planning during a multi-step task.
  - `conversation_history.json`: A log of past interactions and outcomes, helping it understand user intent over time.
  - `learned_preferences.toml`: A file to store inferred user preferences (e.g., `default_export_format = "png"` or `project_archive_location = "sd://nas/archive/"`).

- **Access via CLI:** The agent interacts with its own memory using the standard `sd` CLI, but with special permissions that restrict file operations to its own sandboxed directory. This allows it to "remember" past actions and user feedback to improve its performance on future tasks.

### Agent Permissions and Scopes

To ensure security and user control, agents do not have unrestricted access to the entire VDFS. Each agent operates within a clearly defined **permission scope** for the duration of its task.

- **Scoped Access by Default:** Before an agent is activated, the Agent Manager, in conjunction with the user's request, defines its access rights. The user is prompted for consent if the required permissions are sensitive. For example: _"This agent needs read/write access to your 'Photos' Location to proceed. Allow?"_

- **Types of Permission Scopes:**

  - **Location-Based Scope:** The agent can only read or write within specified Locations (e.g., an "Ingestion Sorter" agent may only access `~/Downloads` and `~/Documents`).
  - **Tag-Based Scope:** The agent's operations are restricted to files matching a certain tag (e.g., an agent can only modify files tagged `#ProjectX`).
  - **Ephemeral Scope:** The agent is only granted permission to operate on a specific list of files returned from an initial search query.

- **Enforcement by the Core:** These permissions are not merely advisory. When the daemon receives a command from an agent-initiated CLI process, the **Transactional Action System** first verifies that the target of the action (e.g., the source and destination paths) falls within the agent's authorized scope. If it does not, the action is rejected before execution.

### The Agentic Loop with User-in-the-Loop Approval

The agent's primary strength is its ability to perform complex discovery and planning, culminating in a single, large-scale action plan that the user can approve. This leverages the **Transactional Action System's** "preview-before-commit" philosophy, ensuring the user is always in control.

**User Prompt:** _"Organize all my photos tagged 'Hawaii Vacation' into folders by year."_

1.  **Discovery and Planning (Observe & Orient):** The agent first runs a series of non-destructive "read" operations to gather all necessary information.

    - It finds all relevant files: `sd search --tag "Hawaii Vacation" --type image`
    - It then iterates through the results, fetching metadata for each: `sd meta get <path> --select exif.date_time`
    - During this phase, the agent builds a complete plan in its internal memory (its "scratchpad"). It determines that 50 files need to be moved into three new directories (`2022`, `2023`, `2024`).

2.  **Formulate a Batch Action (Decide):** Instead of executing 50 individual moves, the agent constructs a **single, batch `FileMoveAction`**. This action encapsulates the entire plan: moving all 50 source files to their calculated final destinations.

3.  **Generate a Preview for Approval (Act & Await):** This is the key step. The agent's first "Act" is not to execute the move, but to ask the system to **simulate** it.

    - It submits the batch action to the **Action System** with a preview flag.
    - The system returns a detailed `ActionPreview`, showing exactly what will happen: which folders will be created, which files will be moved, and that no files will be overwritten.
    - The agent's task status now changes to `AwaitingConfirmation`, and it presents this preview to the user in the UI.

4.  **Execute on User Approval (Final Act):** Once the user reviews the plan and clicks "Confirm," the agent is notified. Only then does it commit the batch action to the durable job queue for execution. The user's approval is the explicit trigger for the final, decisive action.

This workflow is directly supported by the system's data structures, particularly the `TaskStatus` enum, which includes a state specifically for this purpose:

```rust
pub enum TaskStatus {
    Pending,
    Running,
    AwaitingConfirmation(ActionPreview), // Paused for user approval
    Completed,
    Failed(String),
}
```

This ensures that while the agent provides powerful automation, the user always has the final say before any significant changes are made to their files, perfectly aligning with Spacedrive's core principles of safety and user control.

Excellent point. Making the agent's activity transparent is key to building user trust and creating a dynamic user experience.

Here is a new section for the design document that outlines how to achieve this "follow" mode.

---

### Agent Activity Events and UI "Follow" Mode

To make the AI feel like a first-class citizen within the application, the UI needs to be able to observe and react to an agent's activity in real-time. This is accomplished by tagging agent-initiated commands and emitting specific events that the frontend can subscribe to.

- **Agent-Aware CLI Invocation:**
  When the **Execution Coordinator** spawns the `sd` CLI sidecar for an agent, it injects a unique **Task ID** into the new process's environment. When the CLI connects to the core daemon via IPC, it passes this Task ID along with its command. This allows the core to differentiate between commands initiated by a human user and those initiated by a specific agent task.

- **Dedicated Agent Event Stream:**
  Once the core identifies a command as part of an agent task, it emits structured events to the main **EventBus**. The frontend can listen for these specific events to power a "follow mode."

  ```rust
  // Example of events the frontend can subscribe to
  pub enum AgentActivityEvent {
      /// Emitted when an agent's plan is ready for user review.
      AwaitingApproval {
          task_id: Uuid,
          preview: ActionPreview,
      },
      /// Emitted when an agent runs a new command.
      CommandExecuted {
          task_id: Uuid,
          command_string: String, // e.g., "sd search --type image"
      },
      /// Emitted when the agent receives a result from a command.
      CommandResultReceived {
          task_id: Uuid,
          stdout: Vec<String>,
      },
      /// Emitted when the agent provides a final summary.
      TaskCompleted {
          task_id: Uuid,
          summary: String,
      },
  }
  ```

- **Frontend "Follow Mode":**
  By subscribing to this event stream, the frontend can create a rich, interactive experience:

  - **Live Console:** A dedicated panel can show a real-time log of the commands the agent is executing and the results it's receiving.
  - **UI Highlighting:** The main file browser can visually highlight the files and folders the agent is currently processing.
  - **Interactive Prompts:** When an `AwaitingApproval` event is received, the UI can display the `ActionPreview` in a modal, allowing the user to approve or deny the agent's plan.
  - **Progress Indicators:** For long-running batch operations, the UI can display progress bars and status messages, giving the user clear insight into the agent's work.
    Of course. Here is a new section for your Agent Manager design document that explains the asynchronous search architecture and the necessary changes to support it.

---

### 5\. Asynchronous, Agent-Driven Search

To enhance the user experience and create a more scalable, consistent, and responsive system, all search functionality will be unified into a single asynchronous workflow. This design applies to searches initiated by both the AI agent and directly by the user in the UI. It moves away from a direct query-response model and instead leverages the Job System to provide a more robust and interactive search experience.

#### 5.1. The Agent's Role: Initiating and Monitoring

The agent's primary role in the search process is to _initiate_ and _monitor_ the search, rather than directly consuming the results. This creates a clean separation of concerns and allows the agent to continue performing other tasks while the search is in progress.

1.  **New `SearchAction`**: A new `Search(String)` variant will be added to the `Action` enum. The `String` payload will contain the user's natural language search query.
2.  **Dispatching the Action**: To initiate a search, the agent will generate a command for the `sd` CLI, such as:
    ```bash
    sd --action search "find all my photos from my last vacation to Japan"
    ```
3.  **Monitoring Job Status**: The agent will monitor the progress of the search by subscribing to events from the Job System. The agent will receive events like `JobCreated`, `JobInProgress`, and `JobCompleted`, allowing it to track the search without being blocked.

#### 5.2. The Job System: Making Search Asynchronous

The Job System is the core component that enables the asynchronous nature of our search functionality.

1.  **`SearchJob`**: When the `ActionManager` receives a `SearchAction`, it will not execute the search directly. Instead, it will register a new `SearchJob` with the `JobManager`.
2.  **`query_id` Generation**: The `JobManager` will assign a unique `job_id` to the new `SearchJob`. This `job_id` will also serve as the `query_id` for the search, allowing us to track the search and its results throughout the system.

#### 5.3. Caching and Frontend Interaction

This is where the user-facing part of the design comes into play. The frontend is responsible for fetching and rendering the search results, guided by events from the backend.

1.  **Results Caching**: Upon completion, the `SearchJob` will not return the results to the agent. Instead, it will store the results in a cache (e.g., Redis, or a database table), using the `query_id` as the key.
2.  **`SearchResultsReady` Event**: Once the `SearchJob` is complete, the `JobManager` will emit a `SearchResultsReady(query_id)` event. This event will be broadcast to all subscribed clients, including the frontend.
3.  **New GraphQL Endpoint**: The frontend will have a new GraphQL query, `getSearchResults(queryId: ID!)`.
4.  **Rendering the Results**: When the frontend receives the `SearchResultsReady` event (likely via a WebSocket connection), it will use the `query_id` from the event to call the `getSearchResults` GraphQL endpoint. This endpoint will fetch the cached results and the frontend will then render them for the user.

#### 5.4. Required Changes

To support this new asynchronous search architecture, the following changes will be required:

- **Action System (`src/infrastructure/actions/mod.rs`)**:

  - Add a new `Search(String)` variant to the `Action` enum.

- **Job System (`src/infrastructure/jobs/mod.rs`)**:

  - Create a new `SearchJob` type that encapsulates the logic for performing a search and caching the results.
  - The `JobManager` must be able to handle and dispatch `SearchJob`s.
  - A new `SearchResultsReady(query_id)` event must be added to the event system.

- **Caching Layer**:

  - A new caching mechanism will be needed to store search results. The specific implementation (e.g., Redis, in-memory cache) will need to be determined.

- **GraphQL API (`src/interfaces/graphql/mod.rs`)**:

  - A new `getSearchResults(queryId: ID!)` query must be added to the GraphQL schema and resolver. This endpoint will be responsible for fetching the search results from the cache.

- **Frontend**:

  - The frontend must be able to subscribe to backend events (e.g., via WebSockets).
  - The frontend must be updated to handle the `SearchResultsReady` event and call the new `getSearchResults` GraphQL endpoint to render the results.
