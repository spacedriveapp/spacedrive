//! Mounts operations — the share serving indexed sources, and the block
//! cache sitting under it.

pub mod cache_clear;
pub mod cache_status;
pub mod fuse_list;
pub mod fuse_mount;
pub mod fuse_unmount;
pub mod read_trace;
pub mod replication_set_paused;
pub mod replication_status;
pub mod status;
pub mod sync_peers;
pub mod trace_set;

pub use cache_clear::*;
pub use cache_status::*;
pub use fuse_list::*;
pub use fuse_mount::*;
pub use fuse_unmount::*;
pub use read_trace::*;
pub use replication_set_paused::*;
pub use replication_status::*;
pub use status::*;
pub use sync_peers::*;
pub use trace_set::*;
