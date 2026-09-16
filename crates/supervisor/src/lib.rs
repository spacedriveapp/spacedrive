//! Machine-scoped process supervision.
//!
//! One supervisor per host: a service registry, adopt-first startup, health
//! probing with crash restart, per-service log capture, and the host's port
//! ledger. Services are described by [`ServiceKind`] and observed through
//! [`ProcessStatus`]; every state change is broadcast to subscribers.

pub mod compose;
pub mod container;
pub mod exec;
pub mod ledger;
pub mod logs;
pub mod probe;
pub mod spec;
pub mod supervisor;
pub mod timing;

pub use container::{ContainerProvider, ContainerRuntime, RuntimeStatus};
pub use ledger::{
	resolve_port_assignments, LeaseSource, PortAssignment, PortLease, PortLeaseHolder, PortLedger,
	PortLedgerStore, PortPolicy, PortRange, PortRequest, PortResolution,
};
pub use logs::ServiceLogs;
pub use probe::{probe_http, process_alive, ProbeResult};
pub use spec::{
	HealthState, Ownership, ProcessState, ProcessStatus, ServiceDefinition, ServiceKind, SpawnSpec,
};
pub use supervisor::{Supervisor, SupervisorConfig, SupervisorError};
pub use timing::Timing;
