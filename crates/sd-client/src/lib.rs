//! Client library for the Spacedrive daemon.
//!
//! Wraps the daemon's JSON-over-TCP protocol in a typed API: [`CoreClient`]
//! sends `Wire`-registered queries and actions, while [`EventStream`] and
//! [`LogStream`] deliver real-time subscriptions. Rust applications (the CLI,
//! native apps) talk to the daemon through this crate rather than opening
//! sockets themselves.
//!
//! Windowed clients build on two more pieces: [`SubscriptionBroker`] pools
//! event subscriptions so many in-app receivers share one daemon connection
//! per distinct filter, and [`LibraryContext`] carries the current library
//! selection (persisted, watchable) and injects it into calls.

mod broker;
mod client;
mod daemon;
mod library;

pub use broker::{BrokerOptions, BrokerSubscription, SubscriptionBroker};
pub use client::{CoreClient, EventStream, LogStream};
pub use daemon::{
	daemon_binary_name, ensure_daemon, is_daemon_running, DaemonLaunchConfig, EnsureDaemonOutcome,
};
pub use library::LibraryContext;
pub use sd_core::infra::daemon::addr::daemon_socket_addr;
pub use sd_core::infra::daemon::types::{
	DaemonError, DaemonRequest, DaemonResponse, EventFilter, LogFilter,
};
pub use sd_core::infra::event::{log_emitter::LogMessage, Event};
pub use sd_core::infra::wire::Wire;
