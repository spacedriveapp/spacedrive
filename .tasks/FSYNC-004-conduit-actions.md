---
id: FSYNC-004
title: Sync Conduit Management Actions
status: To Do
assignee: unassigned
parent: FSYNC-000
priority: High
tags: [file-sync, actions]
depends_on: [FSYNC-002, ACT-000]
design_doc: docs/core/design/sync/SYNC_CONDUIT_DESIGN.md
---

## Description

Implement LibraryActions for creating, updating, pausing, and deleting sync conduits. These provide the API for managing conduits from the frontend.

## Implementation Notes

Create `src/ops/sync/action.rs`:

```rust
use crate::action::prelude::*;
use super::input::*;

/// Create a new sync conduit
#[derive(Debug, Action)]
#[action(input = SyncConduitCreateInput, output = SyncConduit)]
pub struct SyncConduitCreateAction;

#[async_trait::async_trait]
impl ActionHandler for SyncConduitCreateAction {
    async fn execute(ctx: ActionContext, input: Self::Input) -> ActionResult<Self::Output> {
        // 1. Validate source/destination entries exist
        // 2. Check for conflicting conduits
        // 3. Create sync_relationships record
        // 4. Optionally trigger initial sync
        todo!()
    }
}

/// Update conduit configuration (policy, cadence, etc.)
#[derive(Debug, Action)]
#[action(input = SyncConduitUpdateInput, output = SyncConduit)]
pub struct SyncConduitUpdateAction;

/// Pause/resume a sync conduit
#[derive(Debug, Action)]
#[action(input = SyncConduitToggleInput, output = SyncConduit)]
pub struct SyncConduitToggleAction;

/// Delete a sync conduit
#[derive(Debug, Action)]
#[action(input = SyncConduitDeleteInput, output = ())]
pub struct SyncConduitDeleteAction;
```

## Acceptance Criteria

- [ ] Create action implemented with validation
- [ ] Update action supports policy and config changes
- [ ] Toggle action for pause/resume
- [ ] Delete action with cascade cleanup
- [ ] Actions registered in action system
- [ ] Integration tests for each action

