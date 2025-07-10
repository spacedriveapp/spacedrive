//! Monitoring module for job and event tracking through the daemon
//! 
//! All CLI commands go through the daemon, so we only need daemon-based monitoring.

pub mod daemon_monitor;

// Re-export the monitoring functions
pub use daemon_monitor::{monitor_jobs, monitor_job_by_id};