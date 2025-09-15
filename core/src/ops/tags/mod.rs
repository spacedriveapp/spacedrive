//! Tag operations module
//!
//! This module contains business logic for managing semantic tags,
//! including creation, application, search, and hierarchy management.

pub mod apply;
pub mod create;
pub mod search;
pub mod manager;
pub mod facade;

pub use manager::TagManager;
pub use facade::TaggingFacade;

// Re-export commonly used types
pub use apply::{ApplyTagsAction, ApplyTagsInput, ApplyTagsOutput};
pub use create::{CreateTagAction, CreateTagInput, CreateTagOutput};
pub use search::{SearchTagsAction, SearchTagsInput, SearchTagsOutput};