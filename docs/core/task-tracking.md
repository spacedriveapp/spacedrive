# Spacedrive Task Tracking System

This document outlines the simple, file-based system used to track development tasks, epics, and features for the Spacedrive project. The system is designed to be transparent, developer-friendly, and directly integrated with the codebase and documentation.

## Overview

All work is tracked as individual Markdown files located in the `/.tasks/` directory at the root of the repository. This approach keeps our roadmap and work items version-controlled alongside the code itself.

Each task file uses YAML front matter to define its properties, such as status, assignee, and priority. The structure of this front matter is strictly enforced by a JSON schema (`.tasks/task.schema.json`) to ensure consistency.

## The Task File

Each task is a single `.md` file.

### Naming Convention

Files are named using their unique ID and a URL-friendly slug of their title.

**Format:** `ID-title-slug.md`
**Example:** `CORE-001-entry-centric-model.md`

### YAML Front Matter

The front matter at the top of each file defines the task's attributes. Here are the required and optional fields:

-   `id` (string, required): A unique identifier for the task, composed of a category prefix and a number (e.g., `CORE-001`, `NET-002`).
-   `title` (string, required): The human-readable title of the task. Titles prefixed with "Epic:" represent larger features that group related child tasks.
-   `status` (string, required): The current state of the task. Must be one of `To Do`, `In Progress`, or `Done`.
-   `assignee` (string, required): The person or team responsible for the task (e.g., `james`, `unassigned`).
-   `parent` (string, optional): The `id` of a parent task (an Epic). This creates a hierarchical relationship.
-   `priority` (string, required): The priority level. Must be one of `High`, `Medium`, or `Low`.
-   `tags` (array of strings, optional): A list of relevant tags for filtering and categorization (e.g., `core`, `networking`, `ui`).
-   `whitepaper` (string, optional): A reference to the relevant section in the Spacedrive V2 whitepaper, linking the task directly to the architectural vision.

### Task Content

The body of the Markdown file is used to provide detailed information about the task, such as:

-   **Description**: A high-level overview of the task and its purpose.
-   **Implementation Notes / Steps**: Technical details, proposed solutions, or a checklist of steps to complete the task.
-   **Acceptance Criteria**: A list of conditions that must be met for the task to be considered "Done".

## Tooling and Automation

A custom command-line tool, `task-validator`, exists in the repository to manage and validate these task files.

### Listing Tasks

You can list and filter all tasks using the `list` command. This is useful for getting an overview of the current project status.

**Filter Options:**
- `--status <STATUS>` - Filter by status (e.g., "To Do", "In Progress", "Done")
- `--assignee <ASSIGNEE>` - Filter by assignee
- `--priority <PRIORITY>` - Filter by priority (e.g., "High", "Medium", "Low")
- `--tag <TAG>` - Filter by tag

**Sort Options:**
- `--sort-by <FIELD>` - Sort results by field (id, title, status, priority, assignee)
- `-r, --reverse` - Reverse sort order

**Examples:**
```sh
# List all "In Progress" tasks assigned to "james"
cargo run -p task-validator -- list --status "In Progress" --assignee "james"

# List all tasks sorted by priority (Critical → High → Medium → Low)
cargo run -p task-validator -- list --sort-by priority

# List all "To Do" tasks sorted by ID in reverse order
cargo run -p task-validator -- list --status "To Do" --sort-by id --reverse

# List high priority tasks sorted by assignee
cargo run -p task-validator -- list --priority "High" --sort-by assignee
```

### Validating Tasks

The `validate` command checks all *staged* task files against the JSON schema. This is integrated into a pre-commit git hook, which means that any attempt to commit an invalid or improperly formatted task file will be automatically blocked.

This ensures that all tasks in the repository are consistent and adhere to the defined structure.

## How to Create a New Task

1.  **Find the Category & ID**: Look at the existing files in `/.tasks/` to find the appropriate category prefix (e.g., `CORE`, `NET`, `AI`). Find the highest number in that category and increment it for your new task ID.
2.  **Create the File**: Create a new file in `/.tasks/` using the naming convention (e.g., `NEW-TASK-010-a-brief-description.md`).
3.  **Add Front Matter**: Copy the YAML front matter from a similar task and fill in the details for your new task.
4.  **Write the Description**: Add a clear description and, if possible, implementation notes or acceptance criteria in the markdown body.
5.  **Validate (Optional)**: You can manually run `cargo run -p task-validator -- validate` after staging the file to check it, or simply try to commit and let the pre-commit hook do the validation for you.
