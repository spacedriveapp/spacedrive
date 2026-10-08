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

			// A paired device's volume stands as its owner last reported it;
			// whether that report is current depends on the owner being
			// reachable, which only the device list says.
			let devices: Vec<sd_core::domain::Device> = execute_query!(
				ctx,
				sd_core::ops::devices::list::query::ListLibraryDevicesInput {
					include_offline: true,
					include_details: false,
					show_paired: true,
				}
			);
			let reachable: std::collections::HashSet<uuid::Uuid> = devices
				.iter()
				.filter(|device| device.is_current || device.is_online)
				.map(|device| device.id)
				.collect();

			println!("Tracked {} volume(s):\n", output.volumes.len());

			for volume in output.volumes {
				println!("{}", volume.display_name.as_ref().unwrap_or(&volume.name));
				println!("   ID: {}", volume.id);
				println!("   Fingerprint: {}", volume.fingerprint);
				println!("   Type: {:?}", volume.volume_type);
				println!("   Mount: {}", volume.mount_point.display());
				println!(
					"   Capacity: {} total, {} available",
					format_bytes(volume.total_capacity),
					format_bytes(volume.available_space),
				);
				println!(
					"   Visible: {}, Tracked: {}, State: {}",
					volume.is_user_visible,
					volume.is_tracked,
					volume_state_label(&volume, reachable.contains(&volume.device_id)),
				);
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

/// `mounted`, `unmounted` or `locked` as the volume's owner has it, or
/// `offline` when the owner is a paired device nothing can reach, in which
/// case its last report says nothing about the drive now.
fn volume_state_label(volume: &sd_core::volume::Volume, owner_reachable: bool) -> &'static str {
	if !owner_reachable {
		return "offline";
	}
	volume.state().as_str()
}

fn format_bytes(bytes: u64) -> String {
	const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];
	if bytes == 0 {
		return "0 B".to_string();
	}
	let mut value = bytes as f64;
	let mut unit = 0;
	while value >= 1024.0 && unit < UNITS.len() - 1 {
		value /= 1024.0;
		unit += 1;
	}
	if unit == 0 {
		format!("{} {}", bytes, UNITS[unit])
	} else {
		format!("{:.2} {}", value, UNITS[unit])
	}
}

#[cfg(test)]
mod tests {
	use super::volume_state_label;
	use sd_core::volume::{Volume, VolumeFingerprint};

	fn volume(is_mounted: bool, locked: bool) -> Volume {
		let mut volume = Volume::new(
			uuid::Uuid::nil(),
			VolumeFingerprint("fp".to_string()),
			"vault".to_string(),
			std::path::PathBuf::from("/mnt/vault"),
		);
		volume.is_mounted = is_mounted;
		volume.locked = locked;
		volume
	}

	#[test]
	fn the_state_line_names_the_owners_state_or_the_owner_being_away() {
		assert_eq!(volume_state_label(&volume(true, false), true), "mounted");
		assert_eq!(volume_state_label(&volume(false, false), true), "unmounted");
		assert_eq!(volume_state_label(&volume(false, true), true), "locked");
		assert_eq!(volume_state_label(&volume(false, true), false), "offline");
	}
}
