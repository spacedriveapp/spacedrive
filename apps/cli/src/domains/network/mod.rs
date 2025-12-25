mod args;

use anyhow::Result;
use clap::Subcommand;

use crate::util::prelude::*;

use crate::context::Context;
use sd_core::ops::network::{
	devices::{output::ListPairedDevicesOutput, query::ListPairedDevicesInput},
	pair::{
		cancel::output::PairCancelOutput,
		confirm::output::PairConfirmOutput,
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
	/// Pairing commands
	#[command(subcommand)]
	Pair(PairCmd),
	/// List paired devices
	Devices {
		/// Show only connected devices
		#[arg(long)]
		connected: bool,
	},
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
		NetworkCmd::Pair(pc) => match pc {
			PairCmd::Generate { .. } => {
				let input = pc.to_generate_input().unwrap();
				let out: PairGenerateOutput = execute_action!(ctx, input);
				print_output!(ctx, &out, |o: &PairGenerateOutput| {
					// Show QR code for remote pairing (includes NodeId and relay URL)
					println!("Scan this QR code with your mobile app for remote pairing:");
					println!("┌─────────────────────────────────────────────────────────┐");
					if let Err(e) = qr2term::print_qr(&o.qr_json) {
						println!("Failed to generate QR code: {}", e);
					}
					println!("└─────────────────────────────────────────────────────────┘");
					println!();

					// Show raw QR JSON for debugging
					println!("QR Code JSON (for debugging):");
					println!("   {}", o.qr_json);
					println!();

					// Also show words for manual entry (local pairing)
					println!("Or type these words manually for local pairing:");
					println!("   {}", o.code);

					println!();
					println!("Session: {}", o.session_id);
					println!("Expires at: {}", o.expires_at);
				});

				// Wait for pairing completion or confirmation request
				println!("\nWaiting for device to connect...");
				println!("(Press Ctrl+C to cancel)\n");
				run_pairing_confirmation_loop(ctx, out.session_id).await?;
			}
			PairCmd::Join {
				ref code,
				ref node_id,
			} => {
				// Check if we should run interactive mode
				let input = if let Some(input) = pc.to_join_input() {
					// Non-interactive: code and possibly flags were provided
					input
				} else {
					// Interactive mode: no code provided, enter interactive flow
					run_interactive_pair_join(ctx, code.as_deref()).await?
				};

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
			PairCmd::Confirm { .. } => {
				let input = pc.to_confirm_input().unwrap();
				let out: PairConfirmOutput = execute_action!(ctx, input);
				print_output!(ctx, &out, |o: &PairConfirmOutput| {
					if o.success {
						println!("Pairing confirmed successfully");
					} else {
						println!(
							"Pairing confirmation failed: {}",
							o.error.as_ref().unwrap_or(&"Unknown error".to_string())
						);
					}
				});
			}
		},
		NetworkCmd::Devices { connected } => {
			let input = ListPairedDevicesInput {
				connected_only: connected,
			};
			let out: ListPairedDevicesOutput = execute_core_query!(ctx, input);
			print_output!(ctx, &out, |o: &ListPairedDevicesOutput| {
				if o.devices.is_empty() {
					println!("No paired devices");
					return;
				}
				println!(
					"Paired Devices ({} total, {} connected):",
					o.total, o.connected
				);
				println!("─────────────────────────────────────────────────────");
				for device in &o.devices {
					println!();
					println!("  Name: {}", device.name);
					println!("  ID: {}", device.id);
					println!("  Type: {}", device.device_type);
					println!("  OS Version: {}", device.os_version);
					println!("  App Version: {}", device.app_version);
					println!(
						"  Status: {}",
						if device.is_connected {
							"Connected"
						} else {
							"Paired"
						}
					);
					println!(
						"  Last Seen: {}",
						device.last_seen.format("%Y-%m-%d %H:%M:%S")
					);
				}
			});
		}
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

/// Poll for pairing status and handle confirmation requests
async fn run_pairing_confirmation_loop(ctx: &Context, session_id: uuid::Uuid) -> Result<()> {
	use crate::util::confirm::text;
	use std::io::{self, Write};

	loop {
		// Check pairing status
		let status: PairStatusOutput = execute_core_query!(
			ctx,
			sd_core::ops::network::pair::status::query::PairStatusQueryInput
		);

		// Find our session
		let session = status.sessions.iter().find(|s| s.id == session_id);

		match session {
			Some(s) => {
				// Check state
				match &s.state {
					sd_core::ops::network::pair::status::output::SerializablePairingState::Completed => {
						println!("\n✓ Pairing completed successfully!");
						if let Some(device_name) = &s.remote_device_name {
							println!("  Paired with: {}", device_name);
						}
						break;
					}
					sd_core::ops::network::pair::status::output::SerializablePairingState::AwaitingUserConfirmation {
						confirmation_code,
						expires_at,
					} => {
						// Prompt user for confirmation
						println!("\n═══════════════════════════════════════════════════════");
						println!("  PAIRING REQUEST RECEIVED");
						println!("═══════════════════════════════════════════════════════");
						if let Some(device_name) = &s.remote_device_name {
							println!("  Device: {}", device_name);
						}
						if let Some(device_os) = &s.remote_device_os {
							println!("  OS: {}", device_os);
						}
						println!();
						println!("  Confirmation Code: {}", confirmation_code);
						println!("  Expires at: {}", expires_at);
						println!("═══════════════════════════════════════════════════════");
						println!();

						// Prompt for code
						print!("Enter the code to accept (or 'n' to reject): ");
						io::stdout().flush()?;

						let input = text("", false)?.unwrap_or_default();

						let accepted = if input.to_lowercase() == "n" || input.to_lowercase() == "no" {
							false
						} else if input == *confirmation_code {
							true
						} else {
							println!("Code doesn't match! Rejecting request.");
							false
						};

						// Send confirmation
						let confirm_input = sd_core::ops::network::pair::confirm::input::PairConfirmInput {
							session_id,
							accepted,
						};
						let out: PairConfirmOutput = execute_action!(ctx, confirm_input);

						if !out.success {
							println!(
								"Failed to confirm: {}",
								out.error.as_ref().unwrap_or(&"Unknown error".to_string())
							);
						}

						if !accepted {
							println!("Pairing rejected.");
							break;
						}
					}
					sd_core::ops::network::pair::status::output::SerializablePairingState::Failed { reason } => {
						println!("\n✗ Pairing failed: {}", reason);
						break;
					}
					sd_core::ops::network::pair::status::output::SerializablePairingState::Rejected { reason } => {
						println!("\n✗ Pairing rejected: {}", reason);
						break;
					}
					_ => {
						// Still waiting
						print!(".");
						io::stdout().flush()?;
					}
				}
			}
			None => {
				println!("\n✗ Session not found - may have expired");
				break;
			}
		}

		// Wait before polling again
		tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
	}

	Ok(())
}

async fn run_interactive_pair_join(
	ctx: &Context,
	code: Option<&str>,
) -> Result<sd_core::ops::network::pair::join::input::PairJoinInput> {
	use crate::util::confirm::{select, text};

	println!("\n=== Interactive Pairing ===\n");

	// Ask for pairing code if not provided
	let code = if let Some(c) = code {
		c.to_string()
	} else {
		text("Enter the 12-word pairing code", false)?.unwrap()
	};

	// Get network status to check for relay URL
	let status: sd_core::ops::network::status::NetworkStatus = execute_core_query!(
		ctx,
		sd_core::ops::network::status::query::NetworkStatusQueryInput
	);

	// Ask if they want to use relay for internet pairing
	let use_relay = select(
		"Select pairing mode",
		&[
			"Local network pairing (LAN only)".to_string(),
			"Internet pairing (via relay server)".to_string(),
		],
	)?;

	let (final_code, node_id) = if use_relay == 1 {
		// Internet pairing - need relay info
		println!("\nFor internet pairing, you need:");
		println!("  1. Node ID (from the QR code or pairing output)");
		println!("  2. Session ID (from the QR code or pairing output)");
		if let Some(relay_url) = &status.relay_url {
			println!("  3. Relay URL (default: {})", relay_url);
		} else {
			println!("  3. Relay URL (from the QR code or pairing output)");
		}
		println!();

		let node_id = text("Node ID", false)?.unwrap();
		let session_id = text("Session ID", false)?.unwrap();

		let relay_url = if let Some(default_relay) = &status.relay_url {
			text(&format!("Relay URL (default: {})", default_relay), true)?
				.unwrap_or_else(|| default_relay.clone())
		} else {
			text("Relay URL", false)?.unwrap()
		};

		// Construct QR JSON format
		let code = serde_json::json!({
			"version": 1,
			"words": code,
			"node_id": node_id,
			"relay_url": relay_url,
			"session_id": session_id
		})
		.to_string();

		(code, Some(node_id))
	} else {
		// Local pairing - just use the words
		(code, None)
	};

	Ok(sd_core::ops::network::pair::join::input::PairJoinInput {
		code: final_code,
		node_id,
	})
}
