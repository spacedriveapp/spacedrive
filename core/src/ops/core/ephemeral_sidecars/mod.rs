//! Ephemeral sidecar operations
//!
//! Queries and actions for managing ephemeral sidecars (thumbnails, previews,
//! etc.) for ephemeral entries. Unlike managed sidecars which are persistent
//! and database-tracked, ephemeral sidecars live in temp storage and are
//! queried directly from the filesystem.

pub mod list_query;
pub mod request_action;

pub use list_query::{ListEphemeralSidecarsInput, ListEphemeralSidecarsOutput};
pub use request_action::{RequestEphemeralThumbnailsInput, RequestEphemeralThumbnailsOutput};
