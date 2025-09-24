mod args;

use anyhow::Result;
use clap::Subcommand;

use crate::util::prelude::*;

use crate::context::Context;
use sd_core::ops::network::{
	devices::output::DeviceInfoLite,
	pair::{
		cancel::output::PairCancelOutput,
		generate::output::PairGenerateOutput,
		join::output::PairJoinOutput,
		status::{output::PairStatusOutput, query::PairStatusQuery},
	},
	revoke::output::DeviceRevokeOutput,
	spacedrop::send::output::SpacedropSendOutput,
	status::NetworkStatusQuery,
	DeviceRevokeInput, SpacedropSendInput,
};

use self::args::*;

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
	Revoke(RevokeArgs),
	/// Send files via Spacedrop
	Spacedrop(SpacedropArgs),
}

pub async fn run(ctx: &Context, cmd: NetworkCmd) -> Result<()> {
	match cmd {
		NetworkCmd::Status => {
			let status: sd_core::ops::network::status::NetworkStatus = execute_core_query!(
				ctx,
				sd_core::ops::network::status::query::NetworkStatusQueryInput
			);
			print_output!(
				ctx,
				&status,
				|s: &sd_core::ops::network::status::NetworkStatus| {
					println!(
						"Networking: {}",
						if s.running { "running" } else { "stopped" }
					);
					if let Some(id) = s.node_id.clone() {
						println!("Node ID: {}", id);
					}
					if !s.addresses.is_empty() {
						println!("Addresses:");
						for a in s.addresses.clone() {
							println!("  {}", a);
						}
					}
					println!(
						"Paired: {} | Connected: {}",
						s.paired_devices, s.connected_devices
					);
				}
			);
		}
		NetworkCmd::Devices(args) => {
			let devices: Vec<DeviceInfoLite> = execute_core_query!(ctx, args.to_input());
			print_output!(ctx, &devices, |devs: &Vec<DeviceInfoLite>| {
				if devs.is_empty() {
					println!("No devices found");
					return;
				}
				for d in devs {
					println!(
						"- {} {} ({} | {} | {} | last seen {})",
						d.id,
						d.name,
						d.os_version,
						d.app_version,
						if d.is_connected {
							"connected"
						} else {
							"offline"
						},
						d.last_seen
					);
				}
			});
		}
		NetworkCmd::Pair(pc) => match pc {
			PairCmd::Generate { .. } => {
				let input = pc.to_generate_input().unwrap();
				let out: PairGenerateOutput = execute_action!(ctx, input);
				print_output!(ctx, &out, |o: &PairGenerateOutput| {
					println!("Pairing code: {}", o.code);
					println!("Session: {}", o.session_id);
					println!("Expires at: {}", o.expires_at);
				});
			}
			PairCmd::Join { .. } => {
				let input = pc.to_join_input().unwrap();
				let out: PairJoinOutput = execute_action!(ctx, input);
				print_output!(ctx, &out, |o: &PairJoinOutput| {
					println!("Paired with {} ({})", o.device_name, o.paired_device_id);
				});
			}
			PairCmd::Status => {
				let out: PairStatusOutput = execute_core_query!(
					ctx,
					sd_core::ops::network::pair::status::query::PairStatusQueryInput
				);
				print_output!(ctx, &out, |o: &PairStatusOutput| {
					if o.sessions.is_empty() {
						println!("No pairing sessions");
						return;
					}
					for s in o.sessions.clone() {
						println!("- {} {:?} remote={:?}", s.id, s.state, s.remote_device_id);
					}
				});
			}
			PairCmd::Cancel { .. } => {
				let input = pc.to_cancel_input().unwrap();
				let out: PairCancelOutput = execute_action!(ctx, input);
				print_output!(ctx, &out, |o: &PairCancelOutput| {
					println!("Cancelled: {}", o.cancelled);
				});
			}
		},
		NetworkCmd::Revoke(args) => {
			confirm_or_abort(
				&format!(
					"This will revoke device {} and remove pairing. Continue?",
					args.device_id
				),
				args.yes,
			)?;
			let input: DeviceRevokeInput = args.into();
			let out: DeviceRevokeOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &DeviceRevokeOutput| {
				println!("Revoked: {}", o.revoked);
			});
		}
		NetworkCmd::Spacedrop(args) => {
			let out: SpacedropSendOutput = execute_action!(ctx, SpacedropSendInput::from(args));
			print_output!(ctx, &out, |o: &SpacedropSendOutput| {
				if let Some(j) = o.job_id {
					println!("Transfer job: {}", j);
				}
				if let Some(sid) = o.session_id {
					println!("Spacedrop session: {}", sid);
				}
			});
		}
	}
	Ok(())
}
