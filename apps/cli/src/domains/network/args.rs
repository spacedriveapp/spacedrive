use clap::{Args, Subcommand};
use uuid::Uuid;

use sd_core::{
	domain::addressing::SdPath,
	ops::network::{
		pair::{
			cancel::input::PairCancelInput, generate::input::PairGenerateInput,
			join::input::PairJoinInput,
		},
		revoke::input::DeviceRevokeInput,
		spacedrop::send::input::SpacedropSendInput,
	},
};

#[derive(Subcommand, Debug)]
pub enum PairCmd {
	/// Generate a pairing code (initiator)
	Generate {
		#[arg(long, default_value_t = false)]
		auto_accept: bool,
	},
	/// Join using a pairing code (joiner)
	Join {
		/// Pairing code (12 words or JSON). If not provided, enters interactive mode.
		code: Option<String>,
		/// Relay URL for internet pairing (optional)
		#[arg(long)]
		relay_url: Option<String>,
		/// Node ID for internet pairing (optional, required if relay_url is provided)
		#[arg(long)]
		node_id: Option<String>,
		/// Session ID for internet pairing (optional, required if relay_url is provided)
		#[arg(long)]
		session_id: Option<String>,
	},
	/// Show pairing sessions
	Status,
	/// Cancel a pairing session
	Cancel { session_id: Uuid },
}

impl PairCmd {
	pub fn to_generate_input(&self) -> Option<PairGenerateInput> {
		match self {
			Self::Generate { auto_accept } => Some(PairGenerateInput {
				auto_accept: *auto_accept,
			}),
			_ => None,
		}
	}

	pub fn to_join_input(&self) -> Option<PairJoinInput> {
		match self {
			Self::Join { code, relay_url, node_id, session_id } => {
				// Code is required for non-interactive mode
				let code = code.as_ref()?.clone();

				// If relay URL is provided, construct QR JSON format
				let code = if let Some(relay_url) = relay_url {
					// Validate node_id and session_id are also provided
					let node_id = node_id.as_ref().expect("--node-id is required when --relay-url is provided");
					let session_id = session_id.as_ref().expect("--session-id is required when --relay-url is provided");

					// First try to parse as JSON (in case they passed the full QR JSON)
					if code.trim().starts_with('{') {
						// Already JSON, just use it
						code
					} else {
						// Plain words - construct QR JSON format
						serde_json::json!({
							"version": 1,
							"words": code,
							"node_id": node_id,
							"relay_url": relay_url,
							"session_id": session_id
						}).to_string()
					}
				} else {
					code
				};

				Some(PairJoinInput { code })
			}
			_ => None,
		}
	}

	pub fn to_cancel_input(&self) -> Option<PairCancelInput> {
		match self {
			Self::Cancel { session_id } => Some(PairCancelInput {
				session_id: *session_id,
			}),
			_ => None,
		}
	}
}

#[derive(Args, Debug, Clone)]
pub struct SpacedropArgs {
	/// Target device ID
	pub device_id: Uuid,
	/// Files or directories to share
	pub paths: Vec<String>,
	/// Sender name for display
	#[arg(long)]
	pub sender: Option<String>,
}

impl From<SpacedropArgs> for SpacedropSendInput {
	fn from(args: SpacedropArgs) -> Self {
		let paths = args
			.paths
			.iter()
			.map(|s| SdPath::from_uri(s).unwrap_or_else(|_| SdPath::local(s)))
			.collect();
		Self {
			device_id: args.device_id,
			paths,
			sender: args.sender,
		}
	}
}

#[derive(Args, Debug)]
pub struct RevokeArgs {
	pub device_id: Uuid,
	#[arg(long, short = 'y', default_value_t = false)]
	pub yes: bool,
}

impl From<RevokeArgs> for DeviceRevokeInput {
	fn from(args: RevokeArgs) -> Self {
		Self {
			device_id: args.device_id,
		}
	}
}
