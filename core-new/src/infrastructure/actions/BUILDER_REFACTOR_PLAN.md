# Action Builder Pattern Refactor Plan

## Overview

This refactor introduces a consistent builder pattern for Actions to handle CLI/API input parsing while maintaining domain ownership and type safety. This addresses the current inconsistency between Jobs (decentralized) and Actions (centralized enum) patterns.

## Current State Problems

1. **Input Handling Gap**: Actions need to convert raw CLI/API input to structured domain types
2. **Pattern Inconsistency**: Jobs use dynamic registration, Actions use central enum
3. **Validation Scattered**: No standardized validation approach for action construction
4. **CLI Integration Missing**: No clear path from CLI args to Action types
5. **Inefficient Job Dispatch**: Actions currently use `dispatch_by_name` with JSON serialization instead of direct job creation

## Goals

- Provide fluent builder API for all actions
- Standardize validation at build-time
- Enable seamless CLI/API integration
- Maintain domain ownership of input logic
- Keep serialization compatibility (ActionOutput enum needed like JobOutput)
- Eliminate inefficient `dispatch_by_name` usage in favor of direct job creation

## Implementation Plan

### Phase 1: Infrastructure Foundation

#### 1.1 Create Builder Traits (`src/infrastructure/actions/builder.rs`)

```rust
pub trait ActionBuilder {
    type Action;
    type Error: std::error::Error + Send + Sync + 'static;
    
    fn build(self) -> Result<Self::Action, Self::Error>;
    fn validate(&self) -> Result<(), Self::Error>;
}

pub trait CliActionBuilder: ActionBuilder {
    type Args: clap::Parser;
    
    fn from_cli_args(args: Self::Args) -> Self;
}

#[derive(Debug, thiserror::Error)]
pub enum ActionBuildError {
    #[error("Validation errors: {0:?}")]
    Validation(Vec<String>),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Permission denied: {0}")]
    Permission(String),
}
```

#### 1.2 Create ActionOutput Enum (`src/infrastructure/actions/output.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ActionOutput {
    /// Action completed successfully with no specific output
    Success,
    
    /// Library creation output
    LibraryCreate {
        library_id: Uuid,
        name: String,
    },
    
    /// Library deletion output
    LibraryDelete {
        library_id: Uuid,
    },
    
    /// Folder creation output
    FolderCreate {
        folder_id: Uuid,
        path: PathBuf,
    },
    
    /// File copy dispatch output (action just dispatches to job)
    FileCopyDispatched {
        job_id: Uuid,
        sources_count: usize,
    },
    
    /// File delete dispatch output
    FileDeleteDispatched {
        job_id: Uuid,
        targets_count: usize,
    },
    
    /// Location management outputs
    LocationAdd {
        location_id: Uuid,
        path: PathBuf,
    },
    
    LocationRemove {
        location_id: Uuid,
    },
    
    /// Generic output with custom data
    Custom(serde_json::Value),
}

impl ActionOutput {
    pub fn custom<T: Serialize>(data: T) -> Self {
        Self::Custom(serde_json::to_value(data).unwrap_or(serde_json::Value::Null))
    }
}

impl Default for ActionOutput {
    fn default() -> Self {
        Self::Success
    }
}
```

#### 1.3 Update ActionHandler trait (`src/infrastructure/actions/handler.rs`)

```rust
#[async_trait]
pub trait ActionHandler: Send + Sync {
    async fn validate(
        &self,
        context: Arc<CoreContext>,
        action: &Action,
    ) -> ActionResult<()>;

    async fn execute(
        &self,
        context: Arc<CoreContext>,
        action: Action,
    ) -> ActionResult<ActionOutput>; // Change from ActionReceipt to ActionOutput

    fn can_handle(&self, action: &Action) -> bool;
    fn supported_actions() -> &'static [&'static str];
}
```

### Phase 2: Domain Builder Implementation

For each domain, implement the builder pattern following this template:

#### 2.1 File Copy Action Builder (`src/operations/files/copy/action.rs`)

```rust
pub struct FileCopyActionBuilder {
    sources: Vec<PathBuf>,
    destination: Option<PathBuf>,
    options: CopyOptions,
    errors: Vec<String>,
}

impl FileCopyActionBuilder {
    pub fn new() -> Self { /* ... */ }
    
    // Fluent API methods
    pub fn sources<I, P>(mut self, sources: I) -> Self { /* ... */ }
    pub fn source<P: Into<PathBuf>>(mut self, source: P) -> Self { /* ... */ }
    pub fn destination<P: Into<PathBuf>>(mut self, dest: P) -> Self { /* ... */ }
    pub fn overwrite(mut self, overwrite: bool) -> Self { /* ... */ }
    pub fn verify_checksum(mut self, verify: bool) -> Self { /* ... */ }
    pub fn preserve_timestamps(mut self, preserve: bool) -> Self { /* ... */ }
    pub fn move_files(mut self) -> Self { /* ... */ }
    
    // Validation methods
    fn validate_sources(&mut self) { /* ... */ }
    fn validate_destination(&mut self) { /* ... */ }
}

impl ActionBuilder for FileCopyActionBuilder {
    type Action = FileCopyAction;
    type Error = ActionBuildError;
    
    fn validate(&self) -> Result<(), Self::Error> { /* ... */ }
    fn build(self) -> Result<Self::Action, Self::Error> { /* ... */ }
}

#[derive(clap::Parser)]
pub struct FileCopyArgs {
    pub sources: Vec<PathBuf>,
    #[arg(short, long)]
    pub destination: PathBuf,
    #[arg(long)]
    pub overwrite: bool,
    #[arg(long)]
    pub verify: bool,
    #[arg(long, default_value = "true")]
    pub preserve_timestamps: bool,
    #[arg(long)]
    pub move_files: bool,
}

impl CliActionBuilder for FileCopyActionBuilder {
    type Args = FileCopyArgs;
    
    fn from_cli_args(args: Self::Args) -> Self { /* ... */ }
}

// Convenience methods on the action
impl FileCopyAction {
    pub fn builder() -> FileCopyActionBuilder { /* ... */ }
    pub fn copy_file<S: Into<PathBuf>, D: Into<PathBuf>>(source: S, dest: D) -> FileCopyActionBuilder { /* ... */ }
    pub fn copy_files<I, P, D>(sources: I, dest: D) -> FileCopyActionBuilder { /* ... */ }
}
```

#### 2.2 Domain Handler Updates

Update each action handler to return `ActionOutput` instead of `ActionReceipt` and use direct job dispatch:

```rust
impl ActionHandler for FileCopyHandler {
    async fn execute(
        &self,
        context: Arc<CoreContext>,
        action: Action,
    ) -> ActionResult<ActionOutput> {
        if let Action::FileCopy { library_id, action } = action {
            // Create job instance directly (no JSON roundtrip)
            let sources = action.sources
                .into_iter()
                .map(|path| SdPath::local(path))
                .collect();

            let job = FileCopyJob::new(
                SdPathBatch::new(sources),
                SdPath::local(action.destination)
            ).with_options(action.options);

            // Dispatch job directly
            let job_handle = library.jobs().dispatch(job).await?;
            
            // Return action output instead of receipt
            Ok(ActionOutput::FileCopyDispatched {
                job_id: job_handle.id(),
                sources_count: action.sources.len(),
            })
        } else {
            Err(ActionError::InvalidActionType)
        }
    }
}
```

### Phase 3: CLI Integration

#### 3.1 Create CLI Action Router (`src/infrastructure/actions/cli.rs`)

```rust
pub struct ActionCliRouter;

impl ActionCliRouter {
    pub fn route_and_build(command: &str, args: Vec<String>) -> Result<Action, ActionBuildError> {
        match command {
            "copy" => {
                let args = FileCopyArgs::try_parse_from(args)?;
                let action = FileCopyActionBuilder::from_cli_args(args).build()?;
                Ok(Action::FileCopy { 
                    library_id: get_current_library_id()?, 
                    action 
                })
            }
            "delete" => {
                let args = FileDeleteArgs::try_parse_from(args)?;
                let action = FileDeleteActionBuilder::from_cli_args(args).build()?;
                Ok(Action::FileDelete { 
                    library_id: get_current_library_id()?, 
                    action 
                })
            }
            // ... other commands
            _ => Err(ActionBuildError::Parse(format!("Unknown command: {}", command)))
        }
    }
}
```

#### 3.2 Update CLI Binary (`src/bin/cli.rs`)

```rust
#[derive(clap::Parser)]
enum Commands {
    Copy(FileCopyArgs),
    Delete(FileDeleteArgs),
    // ... other commands
}

async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    let action = match cli.command {
        Commands::Copy(args) => {
            let library_id = get_current_library_id()?;
            let action = FileCopyActionBuilder::from_cli_args(args).build()?;
            Action::FileCopy { library_id, action }
        }
        Commands::Delete(args) => {
            let library_id = get_current_library_id()?;
            let action = FileDeleteActionBuilder::from_cli_args(args).build()?;
            Action::FileDelete { library_id, action }
        }
        // ...
    };
    
    let context = create_core_context().await?;
    let output = context.action_manager().execute(action).await?;
    
    println!("{}", output); // ActionOutput implements Display
    Ok(())
}
```

### Phase 4: API Integration

#### 4.1 Create API Action Parser (`src/infrastructure/actions/api.rs`)

```rust
pub struct ActionApiParser;

impl ActionApiParser {
    pub fn parse_request(
        action_type: &str, 
        params: serde_json::Value,
        library_id: Option<Uuid>
    ) -> Result<Action, ActionBuildError> {
        match action_type {
            "file.copy" => {
                let mut builder = FileCopyActionBuilder::new();
                
                if let Some(sources) = params.get("sources").and_then(|v| v.as_array()) {
                    let sources: Result<Vec<PathBuf>, _> = sources
                        .iter()
                        .map(|v| v.as_str().ok_or("Invalid source").map(PathBuf::from))
                        .collect();
                    builder = builder.sources(sources?);
                }
                
                if let Some(dest) = params.get("destination").and_then(|v| v.as_str()) {
                    builder = builder.destination(dest);
                }
                
                if let Some(overwrite) = params.get("overwrite").and_then(|v| v.as_bool()) {
                    builder = builder.overwrite(overwrite);
                }
                
                let action = builder.build()?;
                Ok(Action::FileCopy { 
                    library_id: library_id.ok_or_else(|| ActionBuildError::Parse("Library ID required".into()))?,
                    action 
                })
            }
            // ... other action types
            _ => Err(ActionBuildError::Parse(format!("Unknown action type: {}", action_type)))
        }
    }
}
```

### Phase 5: Testing Updates

#### 5.1 Builder Tests (`src/operations/files/copy/action.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_builder_fluent_api() {
        let action = FileCopyAction::builder()
            .sources(["/src/file1.txt", "/src/file2.txt"])
            .destination("/dest/")
            .overwrite(true)
            .verify_checksum(true)
            .build()
            .unwrap();
            
        assert_eq!(action.sources.len(), 2);
        assert_eq!(action.destination, PathBuf::from("/dest/"));
        assert!(action.options.overwrite);
        assert!(action.options.verify_checksum);
    }
    
    #[test]
    fn test_builder_validation() {
        let result = FileCopyAction::builder()
            .sources(Vec::<PathBuf>::new()) // Empty sources should fail
            .destination("/dest/")
            .build();
            
        assert!(result.is_err());
        match result.unwrap_err() {
            ActionBuildError::Validation(errors) => {
                assert!(errors.iter().any(|e| e.contains("At least one source")));
            }
            _ => panic!("Expected validation error"),
        }
    }
    
    #[test]
    fn test_cli_integration() {
        let args = FileCopyArgs {
            sources: vec!["/src/file.txt".into()],
            destination: "/dest/".into(),
            overwrite: true,
            verify: false,
            preserve_timestamps: true,
            move_files: false,
        };
        
        let action = FileCopyActionBuilder::from_cli_args(args).build().unwrap();
        assert_eq!(action.sources, vec![PathBuf::from("/src/file.txt")]);
        assert_eq!(action.destination, PathBuf::from("/dest/"));
        assert!(action.options.overwrite);
    }
}
```

#### 5.2 Integration Tests (`tests/action_builder_test.rs`)

```rust
#[tokio::test]
async fn test_action_execution_with_builder() {
    let context = create_test_context().await;
    
    let action = FileCopyAction::builder()
        .source("/test/source.txt")
        .destination("/test/dest.txt")
        .overwrite(true)
        .build()
        .unwrap();
    
    let full_action = Action::FileCopy {
        library_id: test_library_id(),
        action,
    };
    
    let output = context.action_manager().execute(full_action).await.unwrap();
    
    match output {
        ActionOutput::FileCopyDispatched { job_id, sources_count } => {
            assert_eq!(sources_count, 1);
            assert!(!job_id.is_nil());
        }
        _ => panic!("Expected FileCopyDispatched output"),
    }
}
```

## Migration Steps

1. **Create infrastructure** (Phase 1)
2. **Implement FileCopyActionBuilder** as proof of concept
3. **Update FileCopyHandler** to use ActionOutput
4. **Test CLI integration** with file copy
5. **Implement remaining domain builders** (FileDelete, LocationAdd, etc.)
6. **Update all handlers** to use ActionOutput
7. **Complete CLI integration** for all actions
8. **Add API integration**
9. **Update tests** throughout

## Benefits

- **Type Safety**: Build-time validation prevents invalid actions
- **Fluent API**: Easy to use programmatically and from CLI/API
- **Domain Ownership**: Each domain controls its input logic
- **Consistency**: Matches job pattern for serialization needs
- **Extensibility**: Easy to add new actions without infrastructure changes
- **CLI/API Ready**: Direct integration path from external inputs
- **Performance**: Eliminates JSON serialization overhead from `dispatch_by_name`
- **Direct Job Creation**: Actions create job instances directly for better type safety and efficiency

## Backwards Compatibility

- Existing `Action` enum structure remains unchanged
- Current action handlers work with minor output type changes
- Builders are additive - existing construction methods still work
- Migration can be done incrementally, domain by domain