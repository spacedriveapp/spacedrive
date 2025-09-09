//! Location operations

pub mod add;
// pub mod index; // Module removed during migration
pub mod remove;
pub mod rescan;

pub use add::*;
// pub use index::*; // Module removed during migration
pub use remove::*;
pub use rescan::*;