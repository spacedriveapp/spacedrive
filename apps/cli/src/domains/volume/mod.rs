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
	/// List all detected volumes
	List,
	/// Scan for volumes and auto-track eligible ones
	Scan,
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
		VolumeCmd::List => {
			ctx.require_current_library()?;

			let input = sd_core::ops::volumes::list::query::VolumeListQueryInput {
				filter: sd_core::ops::volumes::VolumeFilter::TrackedOnly,
			};
			let output: sd_core::ops::volumes::list::output::VolumeListOutput =
				execute_query!(ctx, input);

			if output.volumes.is_empty() {
				println!("No volumes tracked in the current library.");
				println!("\nVolumes must be detected and tracked by the backend.");
				return Ok(());
			}

			println!("Tracked {} volume(s):\n", output.volumes.len());

			for volume in output.volumes {
				println!("{}", volume.name);
				println!("   ID: {}", volume.id);
				println!("   Fingerprint: {}", volume.fingerprint);
				println!("   Type: {}", volume.volume_type);
				if let Some(mount) = &volume.mount_point {
					println!("   Mount: {}", mount);
				}
				println!();
			}
		}
		VolumeCmd::Scan => {
			println!("Volume scanning must be triggered by the backend.");
			println!("Restart the application to trigger volume detection.");
		}
	}
	Ok(())
}
