mod args;

use anyhow::Result;
use clap::Subcommand;

use crate::context::Context;
use crate::util::prelude::*;

use sd_core::ops::devices::list::{output::LibraryDeviceInfo, query::ListLibraryDevicesInput};

use self::args::*;

#[derive(Subcommand, Debug)]
pub enum DevicesCmd {
	/// List devices from library database
	List(DevicesListArgs),
}

pub async fn run(ctx: &Context, cmd: DevicesCmd) -> Result<()> {
	match cmd {
		DevicesCmd::List(args) => {
			let devices: Vec<LibraryDeviceInfo> = execute_query!(ctx, args.to_input());
			print_output!(ctx, &devices, |devs: &Vec<LibraryDeviceInfo>| {
				if devs.is_empty() {
					println!("No devices found in library database");
					return;
				}

				println!("Devices in library database:");
				println!("");

				for d in devs {
					let status = if d.is_current {
						"*CURRENT*"
					} else if d.is_online {
						"online"
					} else {
						"offline"
					};

					println!("- {} {} ({})", d.id, d.name, status);
					println!("  OS: {} {}", d.os, d.os_version.as_deref().unwrap_or(""));
					if let Some(model) = &d.hardware_model {
						println!("  Hardware: {}", model);
					}
					println!("  Last seen: {}", d.last_seen_at);
					println!("  Created: {}", d.created_at);

					if args.detailed && !d.network_addresses.is_empty() {
						println!("  Network addresses:");
						for addr in &d.network_addresses {
							println!("    {}", addr);
						}
					}

					if args.detailed && d.capabilities.is_some() {
						println!("  Capabilities: {}", d.capabilities.as_ref().unwrap());
					}

					println!("");
				}
			});
		}
	}
	Ok(())
}
