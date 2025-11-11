//! Location operations

pub mod add;
// pub mod index; // Module removed during migration
pub mod list;
pub mod remove;
pub mod rescan;
pub mod suggested;

pub use add::*;
// pub use index::*; // Module removed during migration
pub use list::*;
pub use remove::*;
pub use rescan::*;
pub use suggested::*;
