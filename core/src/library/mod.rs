mod config;
#[allow(clippy::module_inception)]
mod library;
mod manager;
mod name;
mod statistics;

pub use config::*;
pub use library::*;
pub use manager::*;
pub use name::*;
pub use statistics::*;

pub type LibraryId = uuid::Uuid;
