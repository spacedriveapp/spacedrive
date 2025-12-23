use clap::{Args, Subcommand};
use uuid::Uuid;

use sd_core::{
	domain::addressing::SdPath,
	ops::network::{
		pair::{
			cancel::input::PairCancelInput,
			confirm::input::PairConfirmInput,
			generate::input::PairGenerateInput,
			join::input::PairJoinInput,
		},
		revoke::input::DeviceRevokeInput,
		spacedrop::send::input::SpacedropSendInput,
	},
};

#[derive(Subcommand, Debug)]
pub enum PairCmd {
	/// Generate a pairing code (initiator)
	Generate {},
	/// Join using a pairing code (joiner)
	Join {
		/// Pairing code (12 words or JSON). If not provided, enters interactive mode.
		code: Option<String>,
		/// Node ID for remote pairing via pkarr (optional - enables relay path)
		#[arg(long)]
		node_id: Option<String>,
	},
	/// Show pairing sessions
	Status,
	/// Cancel a pairing session
	Cancel { session_id: Uuid },
	/// Confirm a pairing request
	Confirm {
		/// Session ID of the pairing request
		session_id: Uuid,
		/// Accept the pairing request
		#[arg(long, conflicts_with = "reject")]
		accept: bool,
		/// Reject the pairing request
		#[arg(long, conflicts_with = "accept")]
		reject: bool,
	},
}

impl PairCmd {
	pub fn to_generate_input(&self) -> Option<PairGenerateInput> {
		match self {
			Self::Generate {} => Some(PairGenerateInput {}),
			_ => None,
		}
	}

	pub fn to_join_input(&self) -> Option<PairJoinInput> {
		match self {
			Self::Join { code, node_id } => {
				// Code is required for non-interactive mode
				let code = code.as_ref()?.clone();

				// If node_id provided via CLI, wrap in QR JSON format for remote pairing
				let code = if let Some(node_id) = node_id {
					if code.trim().starts_with('{') {
						// Already JSON, just use it
						code
					} else {
						// Plain words + node_id - construct QR JSON format (v2)
						serde_json::json!({
							"version": 2,
							"words": code,
							"node_id": node_id,
						})
						.to_string()
					}
				} else {
					code
				};

				Some(PairJoinInput {
					code,
					node_id: node_id.clone(),
				})
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

	pub fn to_confirm_input(&self) -> Option<PairConfirmInput> {
		match self {
			Self::Confirm {
				session_id,
				accept,
				reject,
			} => {
				// Default to accept if neither flag specified
				let accepted = if *reject { false } else { *accept || true };
				Some(PairConfirmInput {
					session_id: *session_id,
					accepted,
				})
			}
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
