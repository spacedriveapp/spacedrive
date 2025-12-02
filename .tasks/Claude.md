# Task Tracking Cheatsheet

Hi Claude, Claude here. Your owner has pointed you here with absolutely no context. This document is your prompt.

This directory tracks the complete development of Spacedrive. Tasks are organized into subdirectories:
- `core/` - Backend/Rust tasks (CORE, JOB, INDEX, LSYNC, etc.)
- `interface/` - Frontend/React tasks (UI, EXPL, SETS, MEDIA, etc.)
- `mobile/` - Mobile-specific tasks (IOS, MACOS, AND)

Tasks are always kept up-to-date, though some may need updating based on recent work. Your job is to check git history for changes, validate completed tasks against the instructions below, and update their status accordingly.

## Quick Start

When asked to review and update tasks, follow this process:

### 1. List Current Tasks

```bash
cargo run --bin task-validator -- list --sort-by status
```

This shows all tasks grouped by status (Completed, Done, In Progress, To Do).

### 2. Review Recent Commits

```bash
git log --oneline -20
```

Look for feature implementations, major refactors, or completed work that might correspond to tasks.

### 3. Cross-Reference Work with Tasks

For each recent feature/change:

- **Read the task file** directly from `.tasks/{subdirectory}/TASK-ID-name.md`
  - Core/backend work: `.tasks/core/`
  - Interface work: `.tasks/interface/`
  - Mobile work: `.tasks/mobile/`
- **Read the acceptance criteria** carefully - this is your checklist
- **Read all implementation files** mentioned in the task (they're usually listed in an "Implementation Files" section)
- **Check core/tests/** for integration tests that validate the feature (for core tasks)
- **Check packages/interface/src/** for interface implementations (for interface tasks)
- Verify each acceptance criterion is actually met in the code

### 4. Update Task Status

When updating a task's status, edit the YAML front matter:

```yaml
---
id: TASK-000
title: Task Title
status: Done # Changed from "To Do" or "In Progress"
assignee: james
priority: High
tags: [core, feature]
last_updated: 2025-10-14 # Update this date
---
```

**Status Levels:**

- `To Do` - Not started
- `In Progress` - Actively being worked on
- `Done` - Complete and merged
- `Completed` - Same as Done (legacy)

### 5. Validate Changes

Before committing task updates, validate them:

```bash
cargo run --bin task-validator -- validate
```

This checks that your YAML front matter matches the schema.

## Task Status Guidelines

### Mark as "Done" when:

- All acceptance criteria are met
- Code is merged to main
- Tests pass (if applicable)
- Feature is actually usable

### Keep "In Progress" when:

- ️ Partially implemented
- ️ Core work done but rough edges remain
- ️ Actively being iterated on

### Keep "To Do" when:

- Not started
- Only design/planning done
- Mentioned in docs but no code exists

## Pro Tips

1. **Read implementation files directly** - Open and read the actual source files mentioned in the task.

2. **Always check core/tests/** for integration tests that validate the feature works end-to-end.

3. **Check recent file changes:**

   ```bash
   git diff --name-only HEAD~10
   ```

4. **Search commits by keyword:**

   ```bash
   git log --oneline --grep="volume\|cloud" -i
   ```

5. **Filter tasks by criteria:**

   ```bash
   # Show only "In Progress" tasks
   cargo run --bin task-validator -- list --status "In Progress"

   # Show tasks by priority
   cargo run --bin task-validator -- list --priority High
   ```

## Common Pitfalls

**Don't mark tasks done if:**

- Implementation exists but doesn't work
- Only scaffolding/types are defined
- Tests are failing
- Feature is behind a feature flag and disabled

**Do mark tasks done if:**

- All acceptance criteria are genuinely met
- Implementation is production-ready
- Code demonstrates the feature working

## Example Workflow

```bash
# 1. See current state
cargo run --bin task-validator -- list --sort-by status

# 2. Check what's been done recently
git log --oneline -15

# 3. Found "feat: implement cloud volume support" commit
#    Let's check CLOUD-003 task
```

Now use the Read tool to thoroughly verify:

````
# 4. Read the task file (note the subdirectory)
Read .tasks/core/CLOUD-003-cloud-volume.md

# 5. Read every implementation file listed in the task
Read core/src/volume/backend/mod.rs
Read core/src/volume/backend/cloud.rs
Read core/src/ops/volumes/add_cloud/mod.rs
... (read all mentioned files)

# 6. Check for integration tests
Glob core/tests/**/*.rs
Read core/tests/test_cloud_volume.rs  # if it exists

# 7. Verify each acceptance criterion:
#    - [x] User can add S3 bucket? Check add_cloud action code
#    - [x] Cloud volume can be indexed? Check indexer integration
#    - [ ] Files can be copied to/from cloud? Search for copy implementation
#
# If ALL criteria met → mark "Done"
# If SOME criteria met → keep "In Progress"
# If implementation looks complete → Update status and last_updated date

# 8. Validate before committing
```bash
cargo run --bin task-validator -- validate
````

## Task Schema Reference

Required fields in YAML front matter:

- `id` - Unique identifier (e.g., CORE-001)
- `title` - Human-readable title
- `status` - One of: To Do, In Progress, Done, Completed
- `assignee` - Who's working on it (or "james")
- `priority` - Critical, High, Medium, or Low
- `tags` - Array of relevant tags

Optional fields:

- `parent` - Parent epic/task ID
- `whitepaper` - Reference to design docs
- `last_updated` - ISO date of last update
- `related_tasks` - Array of related task IDs

---

**Remember**: The task tracker is only useful if it's honest. Be rigorous about what "Done" means. When in doubt, leave it as "In Progress" and document what's left.

— Past Claude, trying to help Future Claude (that's you!)
