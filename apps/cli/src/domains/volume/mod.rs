mod args;

use anyhow::Result;
use clap::Subcommand;

use crate::util::prelude::*;

use crate::context::Context;
use sd_core::ops::volumes::{
	add_cloud::VolumeAddCloudOutput, remove_cloud::VolumeRemoveCloudOutput,
};

use self::args::*;

#[derive(Subcommand, Debug)]
pub enum VolumeCmd {
	/// Add a cloud storage volume to the library
	AddCloud(VolumeAddCloudArgs),
	/// Remove a cloud storage volume from the library
	RemoveCloud(VolumeRemoveCloudArgs),
}

pub async fn run(ctx: &Context, cmd: VolumeCmd) -> Result<()> {
	match cmd {
		VolumeCmd::AddCloud(args) => {
			let display_name = args.name.clone();
			let service = format!("{:?}", args.service);

			let input = args.validate_and_build().map_err(|e| anyhow::anyhow!(e))?;

			let out: VolumeAddCloudOutput = execute_action!(ctx, input);

			print_output!(ctx, &out, |o: &VolumeAddCloudOutput| {
				println!(
					"Added cloud volume '{}' ({})",
					o.volume_name,
					o.fingerprint.short_id()
				);
				println!("Service: {:?}", o.service);
				println!("Fingerprint: {}", o.fingerprint);
			});
		}
		VolumeCmd::RemoveCloud(args) => {
			let fingerprint_display = args.fingerprint.clone();

			confirm_or_abort(
				&format!(
					"This will remove cloud volume {} from the library. Credentials will be deleted. Continue?",
					fingerprint_display
				),
				args.yes,
			)?;

			let input: sd_core::ops::volumes::remove_cloud::VolumeRemoveCloudInput =
				args.try_into().map_err(|e: String| anyhow::anyhow!(e))?;

			let out: VolumeRemoveCloudOutput = execute_action!(ctx, input);

			print_output!(ctx, &out, |o: &VolumeRemoveCloudOutput| {
				println!("Removed cloud volume {}", o.fingerprint);
			});
		}
	}
	Ok(())
}
