# Universal Action Metadata for Jobs

**Status**: Draft
**Author**: AI Assistant
**Date**: 2024-12-19
**Related Issues**: Job progress events lack context about originating actions

## Summary

This design introduces a universal system for tracking the action that spawned each job, providing rich contextual metadata throughout the job lifecycle. Instead of job-specific solutions (like adding location data only to indexer jobs), this creates a unified approach that works across all job types.

## Problem Statement

Currently, jobs lose connection to their originating action context once dispatched:

- **Limited Context**: Progress events show "Indexing..." but not "Indexing Documents (added location)"
- **No Audit Trail**: Can't trace jobs back to the user action that created them
- **Poor UX**: Generic progress messages instead of contextual information
- **Debugging Difficulty**: Hard to correlate job failures with user actions

### Example Problem

Current indexer progress event:
```json
{
  "job_type": "indexer",
  "progress": 0.99,
  "message": "Finalizing (3846/3877)",
  "metadata": {
    "phase": "Finalizing",
    // No information about what action triggered this
  }
}
```

## Design Goals

1. **Universal**: Works for all job types (indexing, copying, thumbnails, etc.)
2. **Rich Context**: Preserve full action information including inputs and metadata
3. **Backward Compatible**: Doesn't break existing code or APIs
4. **Performance**: Minimal overhead for job dispatch and execution
5. **Extensible**: Easy to add new action types and context fields
6. **Auditable**: Complete trail from user action → job → results

## Architecture

### Core Components

#### 1. ActionContext Structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ActionContext {
    /// The action type that spawned this job
    pub action_type: String,           // e.g., "locations.add", "indexing.scan"

    /// When the action was initiated
    pub initiated_at: DateTime<Utc>,

    /// User/session that triggered the action (if available)
    pub initiated_by: Option<String>,

    /// The original action input (sanitized for security)
    pub action_input: serde_json::Value,

    /// Additional action-specific context
    pub context: serde_json::Value,
}
```

#### 2. Enhanced Job Database Schema

Add action metadata to job records:

```rust
// In core/src/infra/job/database.rs
pub struct Model {
    // ... existing fields ...

    /// Serialized ActionContext
    pub action_context: Option<Vec<u8>>,

    /// Action type for efficient querying
    pub action_type: Option<String>,
}
```

#### 3. ActionContextProvider Trait

```rust
pub trait ActionContextProvider {
    fn create_action_context(&self) -> ActionContext;
    fn action_type_name() -> &'static str;
}
```

### Data Flow

```
User Action (CLI/API/UI)
    ↓
Action::execute()
    ↓
Create ActionContext
    ↓
JobManager::dispatch_with_action(job, context)
    ↓
Store in Job Database
    ↓
Job Progress Events include ActionContext
    ↓
Rich UI/API responses
```

## Implementation Plan

### Phase 1: Core Infrastructure (Week 1)

1. **Create ActionContext struct**
   - `core/src/infra/action/context.rs`
   - Add to action module exports

2. **Database Migration**
   - Add `action_context` and `action_type` fields to jobs table
   - Create migration script

3. **Enhance JobManager**
   - Add `dispatch_with_action()` method
   - Update job creation to store action context
   - Maintain backward compatibility

### Phase 2: Action Integration (Week 2)

1. **Implement ActionContextProvider**
   - Start with high-value actions: `locations.add`, `indexing.scan`
   - Add context creation for each action type

2. **Update Action Execution**
   - Modify action `execute()` methods to use action-aware dispatch
   - Preserve existing dispatch for backward compatibility

### Phase 3: Job Enhancement (Week 2)

1. **Progress Metadata Enhancement**
   - Include action context in job progress metadata
   - Update `ToGenericProgress` implementations

2. **Job Context Propagation**
   - Pass action context through job execution lifecycle
   - Include in job resumption after restart

### Phase 4: API & UI (Week 3)

1. **TypeScript Type Generation**
   - Update specta types for new ActionContext
   - Generate Swift types for companion app

2. **Enhanced Progress Events**
   - Rich job descriptions based on action context
   - Better UI labels and progress messages

## Expected Outcomes

### Enhanced Progress Events

**Before:**
```json
{
  "job_type": "indexer",
  "message": "Finalizing (3846/3877)"
}
```

**After:**
```json
{
  "job_type": "indexer",
  "message": "Finalizing Documents scan (3846/3877)",
  "metadata": {
    "action_context": {
      "action_type": "locations.add",
      "initiated_at": "2024-12-19T10:30:00Z",
      "action_input": {
        "path": "/Users/james/Documents",
        "name": "Documents",
        "mode": "deep"
      },
      "context": {
        "location_id": "550e8400-e29b-41d4-a716-446655440000",
        "operation": "add_location"
      }
    }
  }
}
```

### Action-Specific Examples

#### Location Addition
```json
{
  "action_type": "locations.add",
  "action_input": {
    "path": "/Users/james/Documents",
    "name": "Documents",
    "mode": "deep"
  },
  "context": {
    "location_id": "uuid-here",
    "device_id": "device-uuid",
    "operation": "add_location"
  }
}
```

#### Manual Indexing
```json
{
  "action_type": "indexing.scan",
  "action_input": {
    "paths": ["/home/user/photos"],
    "mode": "content"
  },
  "context": {
    "trigger": "cli_command",
    "operation": "manual_scan"
  }
}
```

#### File Operations
```json
{
  "action_type": "files.copy",
  "action_input": {
    "sources": ["/path/to/file1", "/path/to/file2"],
    "destination": "/target/path"
  },
  "context": {
    "operation": "copy_files",
    "conflict_resolution": "skip"
  }
}
```

## Benefits

### For Users
- **Better Progress Messages**: "Indexing Documents (added location)" vs "Indexing"
- **Context Awareness**: Know why a job is running
- **Troubleshooting**: Understand what action caused issues

### For Developers
- **Complete Audit Trail**: Trace any job back to its originating action
- **Debugging**: Clear causation chain for failures
- **Analytics**: Track which actions generate the most work/failures

### For UIs
- **Rich Display**: Show meaningful job descriptions
- **Smart Filtering**: Group/filter jobs by action type
- **Better UX**: Context-aware progress indication

## Migration & Compatibility

### Backward Compatibility
- `action_context` field is optional in database
- Existing jobs without context continue working normally
- New dispatch methods don't break existing code

### Gradual Adoption
- Actions can implement `ActionContextProvider` incrementally
- Default to existing dispatch for non-enhanced actions
- Progressive enhancement of job descriptions

### Performance Impact
- **Negligible**: ActionContext is small (~100-200 bytes)
- **One-time Cost**: Context created once at job dispatch
- **Query Optimization**: `action_type` field indexed for fast filtering

## Alternative Approaches Considered

### Job-Specific Metadata (e.g., location-only)
- **Limited Scope**: Only works for specific job types
- **Repetitive**: Need different solutions for each job type
- **Maintenance**: Multiple metadata systems to maintain

### Action Logging Separate from Jobs
- **Disconnected**: Hard to correlate actions with jobs
- **Complex Queries**: Need joins across multiple systems
- **Performance**: Additional overhead for correlation

### Universal Action Context (Chosen)
- **Comprehensive**: Works for all current and future job types
- **Unified**: Single system for all action→job relationships
- **Extensible**: Easy to add new action types and context
- **Performance**: Efficient storage and retrieval

## Security Considerations

### Input Sanitization
- Action inputs may contain sensitive data (file paths, user names)
- Implement input sanitization before storing in `action_input`
- Consider separate field for display-safe context

### Access Control
- Action context inherits same access controls as job data
- No additional security surface introduced
- User context (`initiated_by`) respects existing session management

## Future Enhancements

### Phase 2 Features
- **Job Grouping**: Group related jobs by action context
- **Action Replay**: Re-execute failed actions with same context
- **Smart Retry**: Context-aware retry logic for failed jobs

### Analytics & Insights
- **Action Success Rates**: Track which actions fail most often
- **Performance Analysis**: Measure action→job completion times
- **Usage Patterns**: Understand user behavior through action data

### Enhanced UI Features
- **Action-Based Views**: Filter job queues by originating action
- **Context Tooltips**: Rich hover information for jobs
- **Progress Narratives**: Story-like progress descriptions

## Implementation Files

### New Files
- `core/src/infra/action/context.rs` - ActionContext struct and traits
- `docs/core/design/action-metadata-for-jobs.md` - This design document

### Modified Files
- `core/src/infra/job/database.rs` - Schema updates
- `core/src/infra/job/manager.rs` - Enhanced dispatch methods
- `core/src/infra/job/generic_progress.rs` - Metadata enhancement
- `core/src/ops/*/action.rs` - ActionContextProvider implementations

### Migration Files
- `migrations/YYYY-MM-DD-add-action-context-to-jobs.sql` - Database migration

## Success Metrics

- [ ] All major actions provide rich context (locations, indexing, files)
- [ ] Job progress events include meaningful action descriptions
- [ ] UI displays contextual job information
- [ ] Zero performance regression in job dispatch/execution
- [ ] Backward compatibility maintained for all existing code

---

This design provides a comprehensive, extensible foundation for job-action relationships that will improve user experience, debugging capabilities, and system observability across the entire Spacedrive platform.

