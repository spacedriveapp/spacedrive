//! Location operations

pub mod add;
pub mod export;
pub mod import;
pub mod list;
pub mod remove;
pub mod suggested;
pub mod update;
pub mod validate;

pub use add::*;
pub use export::*;
pub use import::*;
pub use list::*;
pub use remove::*;
pub use suggested::*;
pub use update::*;
pub use validate::*;

// Register validation query
crate::register_library_query!(
	validate::ValidateLocationPathQuery,
	"locations.validate_path"
);
