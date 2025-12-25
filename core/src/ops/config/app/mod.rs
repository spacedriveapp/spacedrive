//! App configuration operations
//!
//! Operations for reading and updating daemon-wide application configuration.

pub mod get;
pub mod update;

pub use get::{GetAppConfigQuery, GetAppConfigQueryInput};
pub use update::{UpdateAppConfigAction, UpdateAppConfigInput, UpdateAppConfigOutput};
