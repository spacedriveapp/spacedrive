//! Tag operations module
//!
//! This module contains business logic for managing semantic tags,
//! including creation, application, search, and hierarchy management.

pub mod apply;
pub mod create;
pub mod search;
pub mod semantic_tag_manager;
pub mod semantic_tagging_facade;

pub use semantic_tag_manager::SemanticTagManager;
pub use semantic_tagging_facade::SemanticTaggingFacade;

// Re-export commonly used types
pub use apply::{ApplyTagsAction, ApplyTagsInput, ApplyTagsOutput};
pub use create::{CreateTagAction, CreateTagInput, CreateTagOutput};
pub use search::{SearchTagsAction, SearchTagsInput, SearchTagsOutput};