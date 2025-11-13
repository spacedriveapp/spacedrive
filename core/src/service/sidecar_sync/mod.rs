mod coordinator;
mod filters;

pub use coordinator::SidecarSyncCoordinator;
pub use filters::{
	MissingSidecar, SidecarSource, SidecarSyncFilters, SidecarSyncMode, SidecarTransferPlan,
};
