<!--CREATED: 2025-08-07-->
# Operations Module Refactor Plan

## Current Problems

### 1. **Architectural Issues**

- Mixed abstraction levels in `/operations` (high-level actions, low-level jobs, domain logic)
- Confusing naming: `file_ops` vs `media_processing` vs `indexing`
- Actions are centralized and disconnected from their domains
- Audit logs try to determine library context instead of having it explicit

### 2. **Library Context Issues**

- Actions operate at core level but need library-specific audit logging
- Current `ActionManager.determine_library_id()` is unimplemented placeholder
- No clear separation between global actions (LibraryCreate) and library-scoped actions

### 3. **Domain Modularity Issues**

- Action handlers separated from their domain logic
- No clear ownership of business logic per domain
- Job naming inconsistency (`delete_job.rs` vs `job.rs` in folders)

## Target Architecture

### Core Principles

1. **Domain Modularity**: Each domain owns its complete story (actions + jobs + logic)
2. **Explicit Library Context**: Actions specify library_id when needed
3. **Consistent Structure**: Every domain follows the same pattern
4. **Clear Separation**: Global vs library-scoped actions
5. **Infrastructure vs Operations**: Framework code separate from business logic

### Actions Module Move to Infrastructure

The current `operations/actions/` module should be moved to `infrastructure/actions/` because it provides **framework functionality**, not business logic. This aligns with the existing infrastructure pattern:

**Infrastructure modules provide frameworks/systems:**

- `jobs/` - Job execution framework (traits, manager, registry, executor)
- `events/` - Event system framework (dispatching, handling)
- `database/` - Database access framework (entities, migrations, connections)
- `actions/` - Action dispatch and audit framework (manager, registry, audit logging) 

**Operations modules provide business logic:**

- `files/` - File operation business logic (what to do with files)
- `locations/` - Location management business logic (how to manage locations)
- `indexing/` - Indexing business logic (how to index files)
- `media/` - Media processing business logic (how to process media)

The actions module is pure infrastructure - it doesn't care about the specific business logic of copying files or managing locations. It only provides:

- **ActionManager**: Central dispatch system
- **ActionRegistry**: Auto-discovery of action handlers
- **ActionHandler trait**: Interface for handling actions
- **Audit logging**: Framework for tracking all actions
- **Action enum**: Central registry of all available actions

This creates a clean separation where:

- **Infrastructure** provides the plumbing (how to dispatch, audit, execute)
- **Operations** provides the business logic (what to do with files, locations, etc.)

Each domain operation implements the infrastructure's `ActionHandler` trait, similar to how jobs implement the `Job` trait from `infrastructure/jobs/`. The domain owns the business logic, but uses the infrastructure's framework for execution and audit logging.

### Proposed Structure

```
src/infrastructure/
├── actions/                    # Core action system (framework only)
│   ├── manager.rs             # Central dispatch + audit (fixed library routing)
│   ├── registry.rs            # Auto-discovery via inventory
│   ├── handler.rs             # ActionHandler trait
│   ├── receipt.rs             # ActionReceipt types
│   ├── error.rs               # ActionError types
│   └── mod.rs                 # Core Action enum (references domain actions)
├── jobs/                      # Keep existing
├── events/                    # Keep existing
├── database/                  # Keep existing
└── cli/                       # Keep existing

src/operations/
├── files/                     # Rename from file_ops
│   ├── copy/
│   │   ├── job.rs             # FileCopyJob
│   │   ├── action.rs          # FileCopyAction + handler
│   │   ├── routing.rs         # Keep existing
│   │   └── strategy.rs        # Keep existing
│   ├── delete/                # Convert from delete_job.rs
│   │   ├── job.rs             # FileDeleteJob
│   │   └── action.rs          # FileDeleteAction + handler
│   ├── validation/            # Convert from validation_job.rs
│   │   ├── job.rs             # ValidationJob
│   │   └── action.rs          # ValidationAction + handler
│   ├── duplicate_detection/   # Convert from duplicate_detection_job.rs
│   │   ├── job.rs             # DuplicateDetectionJob
│   │   └── action.rs          # DuplicateDetectionAction + handler
│   └── mod.rs                 # Re-exports
├── locations/                 # Extract from actions/handlers
│   ├── add/
│   │   └── action.rs          # LocationAddAction + handler
│   ├── remove/
│   │   └── action.rs          # LocationRemoveAction + handler
│   ├── index/
│   │   └── action.rs          # LocationIndexAction + handler
│   └── mod.rs                 # Re-exports
├── libraries/                 # Extract from actions/handlers
│   ├── create/
│   │   └── action.rs          # LibraryCreateAction + handler (global scope)
│   ├── delete/
│   │   └── action.rs          # LibraryDeleteAction + handler (global scope)
│   └── mod.rs                 # Re-exports
├── indexing/                  # Keep existing structure + add action.rs
│   ├── job.rs                 # Keep existing IndexerJob
│   ├── action.rs              # NEW: IndexingAction + handler
│   ├── phases/                # Keep existing
│   ├── state.rs               # Keep existing
│   └── ...                    # Keep all existing files
├── content/                   # Keep existing + add action.rs
│   ├── action.rs              # NEW: ContentAction + handler
│   └── mod.rs                 # Keep existing
├── media/                     # Rename from media_processing
│   ├── thumbnails/
│   │   ├── job.rs             # Keep existing ThumbnailJob
│   │   ├── action.rs          # NEW: ThumbnailAction + handler
│   │   └── ...                # Keep existing files
│   └── mod.rs                 # Re-exports
├── metadata/                  # Keep existing + add action.rs
│   ├── action.rs              # NEW: MetadataAction + handler
│   └── mod.rs                 # Keep existing
└── mod.rs                     # Updated job registration
```

## New Action Structure

### Core Action Enum

```rust
// src/infrastructure/actions/mod.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    // Global actions (no library context)
    LibraryCreate(crate::operations::libraries::create::LibraryCreateAction),
    LibraryDelete(crate::operations::libraries::delete::LibraryDeleteAction),

    // Library-scoped actions (require library_id)
    FileCopy {
        library_id: Uuid,
        action: crate::operations::files::copy::FileCopyAction
    },
    FileDelete {
        library_id: Uuid,
        action: crate::operations::files::delete::FileDeleteAction
    },
    FileValidate {
        library_id: Uuid,
        action: crate::operations::files::validation::ValidationAction
    },
    DetectDuplicates {
        library_id: Uuid,
        action: crate::operations::files::duplicate_detection::DuplicateDetectionAction
    },

    LocationAdd {
        library_id: Uuid,
        action: crate::operations::locations::add::LocationAddAction
    },
    LocationRemove {
        library_id: Uuid,
        action: crate::operations::locations::remove::LocationRemoveAction
    },
    LocationIndex {
        library_id: Uuid,
        action: crate::operations::locations::index::LocationIndexAction
    },

    Index {
        library_id: Uuid,
        action: crate::operations::indexing::IndexingAction
    },

    GenerateThumbnails {
        library_id: Uuid,
        action: crate::operations::media::thumbnails::ThumbnailAction
    },

    ContentAnalysis {
        library_id: Uuid,
        action: crate::operations::content::ContentAction
    },

    MetadataOperation {
        library_id: Uuid,
        action: crate::operations::metadata::MetadataAction
    },
}

impl Action {
    pub fn library_id(&self) -> Option<Uuid> {
        match self {
            Action::LibraryCreate(_) | Action::LibraryDelete(_) => None,
            Action::FileCopy { library_id, .. } => Some(*library_id),
            Action::FileDelete { library_id, .. } => Some(*library_id),
            Action::FileValidate { library_id, .. } => Some(*library_id),
            Action::DetectDuplicates { library_id, .. } => Some(*library_id),
            Action::LocationAdd { library_id, .. } => Some(*library_id),
            Action::LocationRemove { library_id, .. } => Some(*library_id),
            Action::LocationIndex { library_id, .. } => Some(*library_id),
            Action::Index { library_id, .. } => Some(*library_id),
            Action::GenerateThumbnails { library_id, .. } => Some(*library_id),
            Action::ContentAnalysis { library_id, .. } => Some(*library_id),
            Action::MetadataOperation { library_id, .. } => Some(*library_id),
        }
    }
}
```

### Fixed ActionManager

```rust
// src/infrastructure/actions/manager.rs
impl ActionManager {
    pub async fn dispatch(
        &self,
        action: Action,
    ) -> ActionResult<ActionReceipt> {
        // 1. Find the correct handler in the registry
        let handler = REGISTRY
            .get(action.kind())
            .ok_or_else(|| ActionError::ActionNotRegistered(action.kind().to_string()))?;

        // 2. Validate the action
        handler.validate(self.context.clone(), &action).await?;

        // 3. Create the initial audit log entry (if library-scoped)
        let audit_entry = if let Some(library_id) = action.library_id() {
            Some(self.create_audit_log(library_id, &action).await?)
        } else {
            None
        };

        // 4. Execute the handler
        let result = handler.execute(self.context.clone(), action).await;

        // 5. Update the audit log with the final status (if we created one)
        if let Some(entry) = audit_entry {
            self.finalize_audit_log(entry, &result).await?;
        }

        result
    }

    // Remove the broken determine_library_id method
    // Library ID is now explicit in the action
}
```

## Migration Steps

### Phase 1: Move Actions to Infrastructure

1. **Move actions module**:

   ```bash
   mv src/operations/actions src/infrastructure/actions
   ```

2. **Update infrastructure mod.rs**:

   ```rust
   pub mod actions;
   pub mod cli;
   pub mod database;
   pub mod events;
   pub mod jobs;
   ```

3. **Update imports** throughout codebase from `crate::operations::actions` to `crate::infrastructure::actions`

### Phase 2: Restructure Domains

1. **Create new domain folders**:

   ```bash
   mkdir -p src/operations/files/{copy,delete,validation,duplicate_detection}
   mkdir -p src/operations/locations/{add,remove,index}
   mkdir -p src/operations/libraries/{create,delete}
   mkdir -p src/operations/media/thumbnails
   ```

2. **Move and rename files**:

   - `file_ops/delete_job.rs` → `files/delete/job.rs`
   - `file_ops/validation_job.rs` → `files/validation/job.rs`
   - `file_ops/duplicate_detection_job.rs` → `files/duplicate_detection/job.rs`
   - `media_processing/` → `media/`

3. **Update imports** throughout codebase

### Phase 3: Extract Domain Actions

1. **Move action handlers to domains**:

   - `infrastructure/actions/handlers/file_copy.rs` → `operations/files/copy/action.rs`
   - `infrastructure/actions/handlers/file_delete.rs` → `operations/files/delete/action.rs`
   - `infrastructure/actions/handlers/location_add.rs` → `operations/locations/add/action.rs`
   - `infrastructure/actions/handlers/location_remove.rs` → `operations/locations/remove/action.rs`
   - `infrastructure/actions/handlers/location_index.rs` → `operations/locations/index/action.rs`
   - `infrastructure/actions/handlers/library_create.rs` → `operations/libraries/create/action.rs`
   - `infrastructure/actions/handlers/library_delete.rs` → `operations/libraries/delete/action.rs`

2. **Create new action files for existing domains**:
   - `operations/indexing/action.rs` (NEW)
   - `operations/content/action.rs` (NEW)
   - `operations/media/thumbnails/action.rs` (NEW)
   - `operations/metadata/action.rs` (NEW)

### Phase 4: Update Core Action System

1. **Refactor Action enum** to use domain-specific types with explicit library_id
2. **Remove handlers directory** (empty after migration)
3. **Update ActionManager** to use explicit library_id from actions
4. **Fix audit log creation** to use correct library database

### Phase 5: Update CLI Integration

1. **Update CLI commands** to pass library_id when creating actions:

   ```rust
   // Before
   let action = Action::FileCopy { sources, destination, options };

   // After
   let library_id = cli_app.get_current_library().await?.id();
   let action = Action::FileCopy {
       library_id,
       action: FileCopyAction { sources, destination, options }
   };
   ```

2. **Update command handlers** to work with new action structure

### Phase 6: Update Job Registration

1. **Update operations/mod.rs** to register jobs from new locations:

   ```rust
   pub fn register_all_jobs() {
       // File operation jobs
       register_job::<files::copy::FileCopyJob>();
       register_job::<files::delete::FileDeleteJob>();
       register_job::<files::validation::ValidationJob>();
       register_job::<files::duplicate_detection::DuplicateDetectionJob>();

       // Other jobs
       register_job::<indexing::IndexerJob>();
       register_job::<media::thumbnails::ThumbnailJob>();
   }
   ```

### Phase 7: Testing and Validation

1. **Update all tests** to use new structure
2. **Run action system tests** to ensure functionality preserved
3. **Test CLI integration** with new action structure
4. **Verify audit logs** are created in correct library databases

## Benefits of This Refactor

### 1. **True Domain Modularity**

- Each domain owns its complete story (actions + jobs + logic)
- Want to understand file operations? Everything is in `files/`
- Want to add location features? Everything is in `locations/`

### 2. **Clear Library Context**

- Actions explicitly specify which library they operate on
- No more guessing or unimplemented library ID determination
- Global actions (library management) clearly separated

### 3. **Consistent Structure**

- Every domain follows the same pattern
- Complex domains: `domain/operation/{job.rs, action.rs}`
- Simple domains: `domain/action.rs`
- No more naming inconsistencies

### 4. **Improved Maintainability**

- Related functionality grouped together
- Clear boundaries between domains
- Easier to test individual domains
- Easier to add new domains

### 5. **Better Developer Experience**

- Intuitive navigation of codebase
- Clear understanding of action vs job responsibilities
- Explicit library context prevents bugs
- Consistent patterns across all domains

## Potential Issues and Solutions

### 1. **Breaking Changes**

- **Issue**: This refactor breaks all existing imports
- **Solution**: Update imports incrementally, test at each phase

### 2. **CLI Integration**

- **Issue**: CLI needs to pass library_id for all actions
- **Solution**: Centralize library ID retrieval in CLI helper functions

### 3. **Action Enum Size**

- **Issue**: Action enum becomes quite large
- **Solution**: This is acceptable for explicit typing, improves type safety

### 4. **Migration Complexity**

- **Issue**: Large number of files to move and update
- **Solution**: Migrate in phases, ensure tests pass at each step

This refactor transforms the operations module from a confusing mix of concerns into a clean, domain-driven architecture where each domain owns its complete functionality and library context is explicit throughout the system.

## Example:

Here's how src/operations/libraries/create/action.rs would look following the Builder Refactor
Plan:

```rust
  //! Library creation action handler

  use crate::{
      context::CoreContext,
      infrastructure::actions::{
          builder::{ActionBuilder, ActionBuildError, CliActionBuilder},
          error::{ActionError, ActionResult},
          handler::ActionHandler,
          output::ActionOutput,
          Action,
      },
      register_action_handler,
  };
  use async_trait::async_trait;
  use clap::Parser;
  use serde::{Deserialize, Serialize};
  use std::{path::PathBuf, sync::Arc};
  use uuid::Uuid;

  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct LibraryCreateAction {
      pub name: String,
      pub path: Option<PathBuf>,
  }

  // Builder implementation
  pub struct LibraryCreateActionBuilder {
      name: Option<String>,
      path: Option<PathBuf>,
      errors: Vec<String>,
  }

  impl LibraryCreateActionBuilder {
      pub fn new() -> Self {
          Self {
              name: None,
              path: None,
              errors: Vec::new(),
          }
      }

      // Fluent API methods
      pub fn name<S: Into<String>>(mut self, name: S) -> Self {
          self.name = Some(name.into());
          self
      }

      pub fn path<P: Into<PathBuf>>(mut self, path: P) -> Self {
          self.path = Some(path.into());
          self
      }

      pub fn auto_path(mut self) -> Self {
          // Use default library path based on OS conventions
          self.path = Some(Self::default_library_path());
          self
      }

      // Validation methods
      fn validate_name(&mut self) {
          if let Some(ref name) = self.name {
              if name.trim().is_empty() {
                  self.errors.push("Library name cannot be empty".to_string());
              }
              if name.len() > 255 {
                  self.errors.push("Library name cannot exceed 255 characters".to_string());
              }
              if name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|']) {
                  self.errors.push("Library name contains invalid characters".to_string());
              }
          } else {
              self.errors.push("Library name is required".to_string());
          }
      }

      fn validate_path(&mut self) {
          if let Some(ref path) = self.path {
              if let Some(parent) = path.parent() {
                  if !parent.exists() {
                      self.errors.push(format!(
                          "Parent directory does not exist: {}",
                          parent.display()
                      ));
                  }
                  if !parent.metadata().map_or(false, |m| m.permissions().readonly()) {
                      // Check if we can write to the parent directory
                  }
              }
          }
      }

      fn default_library_path() -> PathBuf {
          #[cfg(target_os = "macos")]
          {
              dirs::home_dir()
                  .unwrap_or_else(|| PathBuf::from("/tmp"))
                  .join("Library/Application Support/Spacedrive")
          }
          #[cfg(target_os = "windows")]
          {
              dirs::data_dir()
                  .unwrap_or_else(|| PathBuf::from("C:\\ProgramData"))
                  .join("Spacedrive")
          }
          #[cfg(target_os = "linux")]
          {
              dirs::data_dir()
                  .unwrap_or_else(|| PathBuf::from("/tmp"))
                  .join("spacedrive")
          }
      }
  }

  impl ActionBuilder for LibraryCreateActionBuilder {
      type Action = LibraryCreateAction;
      type Error = ActionBuildError;

      fn validate(&self) -> Result<(), Self::Error> {
          let mut builder = self.clone();
          builder.validate_name();
          builder.validate_path();

          if !builder.errors.is_empty() {
              return Err(ActionBuildError::Validation(builder.errors));
          }

          Ok(())
      }

      fn build(self) -> Result<Self::Action, Self::Error> {
          self.validate()?;

          Ok(LibraryCreateAction {
              name: self.name.unwrap(), // Safe after validation
              path: self.path,
          })
      }
  }

  // CLI Integration
  #[derive(Parser)]
  pub struct LibraryCreateArgs {
      /// Name for the new library
      pub name: String,

      /// Path where the library should be created
      #[arg(short, long)]
      pub path: Option<PathBuf>,

      /// Use automatic path based on OS conventions
      #[arg(long)]
      pub auto_path: bool,
  }

  impl CliActionBuilder for LibraryCreateActionBuilder {
      type Args = LibraryCreateArgs;

      fn from_cli_args(args: Self::Args) -> Self {
          let mut builder = Self::new().name(args.name);

          if args.auto_path {
              builder = builder.auto_path();
          } else if let Some(path) = args.path {
              builder = builder.path(path);
          }

          builder
      }
  }

  // Convenience methods on the action
  impl LibraryCreateAction {
      pub fn builder() -> LibraryCreateActionBuilder {
          LibraryCreateActionBuilder::new()
      }

      /// Quick constructor for library with auto path
      pub fn new_auto<S: Into<String>>(name: S) -> LibraryCreateActionBuilder {
          Self::builder().name(name).auto_path()
      }

      /// Quick constructor for library with custom path
      pub fn new_at<S: Into<String>, P: Into<PathBuf>>(
          name: S,
          path: P,
      ) -> LibraryCreateActionBuilder {
          Self::builder().name(name).path(path)
      }
  }

  // Handler implementation
  pub struct LibraryCreateHandler;

  impl LibraryCreateHandler {
      pub fn new() -> Self {
          Self
      }
  }

  #[async_trait]
  impl ActionHandler for LibraryCreateHandler {
      async fn validate(
          &self,
          _context: Arc<CoreContext>,
          action: &Action,
      ) -> ActionResult<()> {
          if let Action::LibraryCreate(action) = action {
              // Additional runtime validation (builder already did static validation)
              if action.name.trim().is_empty() {
                  return Err(ActionError::Validation {
                      field: "name".to_string(),
                      message: "Library name cannot be empty".to_string(),
                  });
              }

              // Check if library name already exists
              // TODO: Implement library name uniqueness check

              Ok(())
          } else {
              Err(ActionError::InvalidActionType)
          }
      }

      async fn execute(
          &self,
          context: Arc<CoreContext>,
          action: Action,
      ) -> ActionResult<ActionOutput> {
          if let Action::LibraryCreate(action) = action {
              let library_manager = &context.library_manager;

              // Create the library (this is an immediate operation, not a background job)
              let new_library = library_manager
                  .create_library(action.name.clone(), action.path.clone())
                  .await
                  .map_err(|e| ActionError::Internal(e.to_string()))?;

              // Return structured output instead of generic JSON
              Ok(ActionOutput::LibraryCreate {
                  library_id: new_library.id(),
                  name: action.name,
              })
          } else {
              Err(ActionError::InvalidActionType)
          }
      }

      fn can_handle(&self, action: &Action) -> bool {
          matches!(action, Action::LibraryCreate(_))
      }

      fn supported_actions() -> &'static [&'static str] {
          &["library.create"]
      }
  }

  // Register this handler
  register_action_handler!(LibraryCreateHandler, "library.create");

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn test_builder_fluent_api() {
          let action = LibraryCreateAction::builder()
              .name("My Library")
              .path("/home/user/libraries/my-library")
              .build()
              .unwrap();

          assert_eq!(action.name, "My Library");
          assert_eq!(action.path, Some(PathBuf::from("/home/user/libraries/my-library")));
      }

      #[test]
      fn test_builder_validation() {
          // Empty name should fail
          let result = LibraryCreateAction::builder()
              .name("")
              .build();

          assert!(result.is_err());
          match result.unwrap_err() {
              ActionBuildError::Validation(errors) => {
                  assert!(errors.iter().any(|e| e.contains("cannot be empty")));
              }
              _ => panic!("Expected validation error"),
          }

          // Invalid characters should fail
          let result = LibraryCreateAction::builder()
              .name("Library/With*Invalid:Characters")
              .build();

          assert!(result.is_err());
      }

      #[test]
      fn test_cli_integration() {
          let args = LibraryCreateArgs {
              name: "Test Library".to_string(),
              path: Some("/custom/path".into()),
              auto_path: false,
          };

          let action = LibraryCreateActionBuilder::from_cli_args(args).build().unwrap();
          assert_eq!(action.name, "Test Library");
          assert_eq!(action.path, Some(PathBuf::from("/custom/path")));
      }

      #[test]
      fn test_auto_path() {
          let args = LibraryCreateArgs {
              name: "Test Library".to_string(),
              path: None,
              auto_path: true,
          };

          let action = LibraryCreateActionBuilder::from_cli_args(args).build().unwrap();
          assert_eq!(action.name, "Test Library");
          assert!(action.path.is_some()); // Should have auto-generated path
      }

      #[test]
      fn test_convenience_constructors() {
          // Auto path constructor
          let action = LibraryCreateAction::new_auto("Auto Library").build().unwrap();
          assert_eq!(action.name, "Auto Library");
          assert!(action.path.is_some());

          // Custom path constructor
          let action = LibraryCreateAction::new_at("Custom Library", "/custom/path")
              .build()
              .unwrap();
          assert_eq!(action.name, "Custom Library");
          assert_eq!(action.path, Some(PathBuf::from("/custom/path")));
      }
  }
```

Key Features Added

1. Builder Pattern

```rust
  let action = LibraryCreateAction::builder()
      .name("My Library")
      .path("/custom/path")
      .build()?;
```

2. CLI Integration

```rust
  #[derive(Parser)]
  pub struct LibraryCreateArgs {
      pub name: String,
      #[arg(short, long)]
      pub path: Option<PathBuf>,
      #[arg(long)]
      pub auto_path: bool,
  }
```

3. Validation at Build Time

- Empty name validation
- Invalid character checking
- Path existence validation
- Length limits

4. Convenience Methods

```rust
  // Quick constructors
  LibraryCreateAction::new_auto("Library Name")
  LibraryCreateAction::new_at("Library Name", "/path")
```

5. Structured Output

```rust
  Ok(ActionOutput::LibraryCreate {
      library_id: new_library.id(),
      name: action.name,
  })
```

6. Comprehensive Tests

- Builder validation
- CLI argument parsing
- Fluent API usage
- Convenience constructors

This follows all the patterns from the refactor plan while being specifically tailored to
library creation needs!
