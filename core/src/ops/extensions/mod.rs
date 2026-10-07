//! Extension operations
//!
//! Lists the WASM extensions the daemon loaded from `<data dir>/extensions`
//! and starts the jobs they registered. Both ops exist in every build so the
//! wire surface and generated client types do not change with the `wasm`
//! feature; without it, the list is empty and a run is refused.

mod list;
mod run_job;

pub use list::*;
pub use run_job::*;
