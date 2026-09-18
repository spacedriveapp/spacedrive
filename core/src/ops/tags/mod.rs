//! Tag operations.
//!
//! Definitions and assertions live in each source store
//! (`sd_store::tags`); the library holds only definitions applied nowhere
//! yet. These operations resolve targets through the volume index, write
//! through store handles, and stamp every claim with an HLC and the device
//! that made it. `docs/plans/2026-09-17-tags-on-source-stores.md` is the
//! execution plan.

pub mod ancestors;
pub mod apply;
pub mod by_id;
pub mod children;
pub mod create;
pub mod decorate;
pub mod definitions;
pub mod delete;
pub mod files_by_tag;
pub mod merge;
pub mod outbox;
pub mod search;
pub mod stamp;
pub mod targets;
pub mod unapply;

pub use apply::{ApplyTagsAction, ApplyTagsInput, ApplyTagsOutput, TagTargets};
pub use create::{CreateTagAction, CreateTagInput, CreateTagOutput};
pub use delete::{DeleteTagAction, DeleteTagInput, DeleteTagOutput};
pub use search::{SearchTagsInput, SearchTagsOutput, SearchTagsQuery};
pub use unapply::{UnapplyTagsAction, UnapplyTagsInput, UnapplyTagsOutput};
