//! Machine-scoped process supervision ops.
//!
//! The host runs one supervisor, owned by [`manager::ProcessManager`]. These
//! ops are the only mutation path: register a service, start and stop it,
//! read its logs, and inspect the host's port ledger.

pub mod leases;
pub mod list;
pub mod logs;
pub mod manager;
pub mod register;
pub mod resource;
pub mod start;
pub mod stop;

pub use manager::ProcessManager;
