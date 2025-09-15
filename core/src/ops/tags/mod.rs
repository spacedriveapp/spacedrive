//! Semantic tag operations
//!
//! This module provides action implementations for the semantic tagging system.
//! These actions integrate with the Action System for validation, audit logging,
//! and transactional operations.

pub mod apply;
pub mod create;
pub mod search;

// Re-export commonly used types
pub use apply::{ApplyTagsAction, ApplyTagsInput, ApplyTagsOutput};
pub use create::{CreateTagAction, CreateTagInput, CreateTagOutput};
pub use search::{SearchTagsAction, SearchTagsInput, SearchTagsOutput};