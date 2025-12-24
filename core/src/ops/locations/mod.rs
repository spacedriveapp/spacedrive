//! Location operations

pub mod add;
pub mod enable_indexing;
pub mod export;
pub mod import;
pub mod list;
pub mod remove;
pub mod rescan;
pub mod service_settings;
pub mod suggested;
pub mod trigger_job;
pub mod update;
pub mod validate;

pub use add::*;
pub use enable_indexing::*;
pub use export::*;
pub use import::*;
pub use list::*;
pub use remove::*;
pub use rescan::*;
pub use service_settings::*;
pub use suggested::*;
pub use trigger_job::*;
pub use update::*;
pub use validate::*;

// Register validation query
crate::register_library_query!(
	validate::ValidateLocationPathQuery,
	"locations.validate_path"
);
