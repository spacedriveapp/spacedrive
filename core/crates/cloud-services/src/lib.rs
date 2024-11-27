#![recursion_limit = "256"]
#![warn(
	clippy::all,
	clippy::pedantic,
	clippy::correctness,
	clippy::perf,
	clippy::style,
	clippy::suspicious,
	clippy::complexity,
	clippy::nursery,
	clippy::unwrap_used,
	unused_qualifications,
	rust_2018_idioms,
	trivial_casts,
	trivial_numeric_casts,
	unused_allocation,
	clippy::unnecessary_cast,
	clippy::cast_lossless,
	clippy::cast_possible_truncation,
	clippy::cast_possible_wrap,
	clippy::cast_precision_loss,
	clippy::cast_sign_loss,
	clippy::dbg_macro,
	clippy::deprecated_cfg_attr,
	clippy::separated_literal_suffix,
	deprecated
)]
#![forbid(deprecated_in_future)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

mod error;

mod client;
mod key_manager;
mod p2p;
mod sync;
mod token_refresher;

pub use client::CloudServices;
pub use error::{Error, GetTokenError};
pub use key_manager::KeyManager;
pub use p2p::{
	CloudP2P, JoinSyncGroupResponse, JoinedLibraryCreateArgs, NotifyUser, Ticket, UserResponse,
};
pub use sync::{
	declare_actors as declare_cloud_sync, SyncActors as CloudSyncActors,
	SyncActorsState as CloudSyncActorsState,
};

// Re-exports
pub use quic_rpc::transport::quinn::QuinnConnector;

// Export URL for the auth server
pub const AUTH_SERVER_URL: &str = "https://auth.spacedrive.com";
// pub const AUTH_SERVER_URL: &str = "http://localhost:9420";
