use anyhow::Result;
use clap::{Parser, Subcommand};
use uuid::Uuid;

use crate::context::{Context, OutputFormat};
use crate::util::output::print_json;

#[derive(Parser, Debug, Clone)]
pub struct NetworkDevicesArgs {
	/// Only show paired devices
	#[arg(long, default_value_t = false)]
	pub paired_only: bool,
	/// Only show connected devices
	#[arg(long, default_value_t = false)]
	pub connected_only: bool,
}

#[derive(Subcommand, Debug)]
pub enum PairCmd {
	/// Generate a pairing code (initiator)
	Generate { #[arg(long, default_value_t = false)] auto_accept: bool },
	/// Join using a pairing code (joiner)
	Join { code: String },
	/// Show pairing sessions
	Status,
	/// Cancel a pairing session
	Cancel { session_id: Uuid },
}

#[derive(Parser, Debug, Clone)]
pub struct SpacedropArgs {
	/// Target device ID
	pub device_id: Uuid,
	/// Files or directories to share
	pub paths: Vec<String>,
	/// Sender name for display
	#[arg(long)]
	pub sender: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum NetworkCmd {
	/// Show networking status
	Status,
	/// List devices
	Devices(NetworkDevicesArgs),
	
	/// Pairing commands
	#[command(subcommand)]
	Pair(PairCmd),
	/// Revoke a paired device
	Revoke { device_id: Uuid },
	/// Send files via Spacedrop
	Spacedrop(SpacedropArgs),
}

pub async fn run(ctx: &Context, cmd: NetworkCmd) -> Result<()> {
	use sd_core::ops::network::*;
	match cmd {
		NetworkCmd::Status => {
			let status: NetworkStatus = ctx.core.query(&NetworkStatusQuery).await?;
			match ctx.format {
				OutputFormat::Human => {
					println!("Networking: {}", if status.running { "running" } else { "stopped" });
					if let Some(id) = status.node_id { println!("Node ID: {}", id); }
					if !status.addresses.is_empty() {
						println!("Addresses:");
						for a in status.addresses { println!("  {}", a); }
					}
					println!("Paired: {} | Connected: {}", status.paired_devices, status.connected_devices);
				}
				OutputFormat::Json => print_json(&status),
			}
		}
		NetworkCmd::Devices(args) => {
			let q = if args.connected_only { ListDevicesQuery::connected() } else if args.paired_only { ListDevicesQuery::paired() } else { ListDevicesQuery::all() };
			let devices: Vec<DeviceInfoLite> = ctx.core.query(&q).await?;
			match ctx.format {
				OutputFormat::Human => {
					if devices.is_empty() { println!("No devices found"); }
					for d in devices { println!("- {} {} ({} | {} | {} | last seen {})", d.id, d.name, d.os_version, d.app_version, if d.is_connected { "connected" } else { "offline" }, d.last_seen); }
				}
				OutputFormat::Json => print_json(&devices),
			}
		}
		
		NetworkCmd::Pair(pc) => match pc {
			PairCmd::Generate { auto_accept } => {
				let bytes = ctx.core.action(&PairGenerateInput { auto_accept }).await?;
				let out: PairGenerateOutput = bincode::deserialize(&bytes)?;
				match ctx.format {
					OutputFormat::Human => {
						println!("Pairing code: {}", out.code);
						println!("Session: {}", out.session_id);
						println!("Expires at: {}", out.expires_at);
					}
					OutputFormat::Json => print_json(&out),
				}
			}
			PairCmd::Join { code } => {
				let bytes = ctx.core.action(&PairJoinInput { code }).await?;
				let out: PairJoinOutput = bincode::deserialize(&bytes)?;
				match ctx.format {
					OutputFormat::Human => println!("Paired with {} ({})", out.device_name, out.paired_device_id),
					OutputFormat::Json => print_json(&out),
				}
			}
			PairCmd::Status => {
				let out: PairStatusOutput = ctx.core.query(&PairStatusQuery).await?;
				match ctx.format {
					OutputFormat::Human => {
						if out.sessions.is_empty() { println!("No pairing sessions"); }
						for s in out.sessions { println!("- {} {:?} remote={:?}", s.id, s.state, s.remote_device_id); }
					}
					OutputFormat::Json => print_json(&out),
				}
			}
			PairCmd::Cancel { session_id } => {
				let bytes = ctx.core.action(&PairCancelInput { session_id }).await?;
				let out: PairCancelOutput = bincode::deserialize(&bytes)?;
				println!("Cancelled: {}", out.cancelled);
			}
		},
		NetworkCmd::Revoke { device_id } => {
			let bytes = ctx.core.action(&DeviceRevokeInput { device_id }).await?;
			let out: DeviceRevokeOutput = bincode::deserialize(&bytes)?;
			println!("Revoked: {}", out.revoked);
		}
		NetworkCmd::Spacedrop(args) => {
			use sd_core::domain::addressing::SdPath;
			let paths = args.paths.iter().map(|s| SdPath::from_uri(s).unwrap_or_else(|_| SdPath::local(s))).collect::<Vec<_>>();
			let bytes = ctx.core.action(&SpacedropSendInput { device_id: args.device_id, paths, sender: args.sender }).await?;
			let out: SpacedropSendOutput = bincode::deserialize(&bytes)?;
			match ctx.format {
				OutputFormat::Human => {
					if let Some(j) = out.job_id { println!("Transfer job: {}", j); }
					if let Some(sid) = out.session_id { println!("Spacedrop session: {}", sid); }
				}
				OutputFormat::Json => print_json(&out),
			}
		}
	}
	Ok(())
}

