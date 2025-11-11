//! Space operations
//!
//! Queries and actions for managing Spaces, SpaceGroups, and SpaceItems

pub mod add_group;
pub mod add_item;
pub mod create;
pub mod delete;
pub mod delete_group;
pub mod delete_item;
pub mod get;
pub mod get_layout;
pub mod list;
pub mod reorder;
pub mod update;
pub mod update_group;

pub use add_group::*;
pub use add_item::*;
pub use create::*;
pub use delete::*;
pub use delete_group::*;
pub use delete_item::*;
pub use get::*;
pub use get_layout::*;
pub use list::*;
pub use reorder::*;
pub use update::*;
pub use update_group::*;
