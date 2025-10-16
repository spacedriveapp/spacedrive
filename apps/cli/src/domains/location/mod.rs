mod args;

use anyhow::Result;
use clap::Subcommand;

use crate::util::prelude::*;

use crate::context::Context;
use sd_core::ops::locations::{
	add::{action::LocationAddInput, output::LocationAddOutput},
	list::{output::LocationsListOutput, query::LocationsListQueryInput},
	remove::output::LocationRemoveOutput,
	rescan::output::LocationRescanOutput,
};

use self::args::*;

#[derive(Subcommand, Debug)]
pub enum LocationCmd {
	/// Add a new location to the library
	Add(LocationAddArgs),
	/// List all locations in the library
	List,
	/// Remove a location from the library
	Remove(LocationRemoveArgs),
	/// Rescan a location
	Rescan(LocationRescanArgs),
}

pub async fn run(ctx: &Context, cmd: LocationCmd) -> Result<()> {
	match cmd {
		LocationCmd::Add(args) => {
			let input = if args.is_interactive() {
				// Interactive mode
				run_interactive_add(ctx).await?
			} else {
				// Non-interactive mode
				let sd_path = args.build_sd_path()?;
				let mode = args
					.mode
					.map(|m| m.into())
					.unwrap_or(sd_core::ops::indexing::IndexMode::Content);

				LocationAddInput {
					path: sd_path,
					name: args.name,
					mode,
				}
			};

			let out: LocationAddOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &LocationAddOutput| {
				println!("Added location {} -> {}", o.location_id, o.path);
			});
		}
		LocationCmd::List => {
			let out: sd_core::ops::locations::list::output::LocationsListOutput =
				execute_query!(ctx, LocationsListQueryInput {});
			print_output!(ctx, &out, |o: &LocationsListOutput| {
				if o.locations.is_empty() {
					println!("No locations found");
					return;
				}
				for loc in &o.locations {
					println!("- {} {}", loc.id, loc.path.display());
				}
			});
		}
		LocationCmd::Remove(args) => {
			confirm_or_abort(
				&format!(
					"This will remove location {} from the library. Continue?",
					args.location_id
				),
				args.yes,
			)?;
			let input: sd_core::ops::locations::remove::action::LocationRemoveInput = args.into();
			let out: LocationRemoveOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &LocationRemoveOutput| {
				println!("Removed location {}", o.location_id);
			});
		}
		LocationCmd::Rescan(args) => {
			let input: sd_core::ops::locations::rescan::action::LocationRescanInput = args.into();
			let out: LocationRescanOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &LocationRescanOutput| {
				println!("Rescan requested for {}", o.location_id);
			});
		}
	}
	Ok(())
}

async fn run_interactive_add(ctx: &Context) -> Result<LocationAddInput> {
	use crate::util::confirm::{select, text};
	use sd_core::domain::addressing::SdPath;
	use sd_core::ops::indexing::IndexMode;

	println!("\n=== Add New Location ===\n");

	// 1. Location type
	let location_type = select(
		"What type of location would you like to add?",
		&["Local filesystem".to_string(), "Cloud storage".to_string()],
	)?;

	let sd_path = if location_type == 0 {
		// Local filesystem
		let path_str = text("Enter the local path", false)?.unwrap();
		let path_buf = std::path::PathBuf::from(path_str);

		// Validate that path exists
		if !path_buf.exists() {
			anyhow::bail!("Path does not exist: {}", path_buf.display());
		}
		if !path_buf.is_dir() {
			anyhow::bail!("Path must be a directory: {}", path_buf.display());
		}

		SdPath::local(path_buf)
	} else {
		// Cloud storage
		use sd_core::ops::volumes::list::VolumeListQueryInput;

		let volumes: sd_core::ops::volumes::list::VolumeListOutput =
			execute_query!(ctx, VolumeListQueryInput {});

		if volumes.volumes.is_empty() {
			anyhow::bail!(
				"No cloud volumes found. Add a cloud volume first with:\n  sd volume add-cloud"
			);
		}

		// Present volume choices (showing service-based URIs)
		let volume_choices: Vec<String> = volumes
			.volumes
			.iter()
			.filter_map(|v| {
				// Parse mount_point to get service and identifier
				v.mount_point.as_ref().map(|mp| {
					format!(
						"{} ({}) - {}",
						v.name,
						mp, // Show mount point like "s3://bucket"
						v.volume_type
					)
				})
			})
			.collect();

		if volume_choices.is_empty() {
			anyhow::bail!(
				"No cloud volumes with mount points found. Cloud volumes must have mount points."
			);
		}

		let volume_idx = select("Select cloud volume", &volume_choices)?;

		// Get the mount point for the selected volume
		let selected_volume = &volumes.volumes[volume_idx];
		let mount_point = selected_volume
			.mount_point
			.as_ref()
			.ok_or_else(|| anyhow::anyhow!("Selected volume has no mount point"))?;

		// Get cloud path within the volume
		let cloud_path = text("Enter path within volume (e.g., / for root or /photos)", false)?.unwrap();

		// Construct service-based URI: mount_point + path
		let full_uri = if cloud_path.starts_with('/') {
			format!("{}{}", mount_point, cloud_path)
		} else {
			format!("{}/{}", mount_point, cloud_path)
		};

		// Parse the URI to create SdPath
		SdPath::from_uri(&full_uri)
			.map_err(|e| anyhow::anyhow!("Failed to parse cloud path: {}", e))?
	};

	// 2. Name (optional)
	let name = text("Location name", true)?;

	// 3. Index mode
	let mode_idx = select(
		"Select indexing mode",
		&[
			"Content (recommended - indexes file metadata and content hashes)".to_string(),
			"Shallow (metadata only - faster)".to_string(),
			"Deep (full analysis - slowest)".to_string(),
		],
	)?;

	let mode = match mode_idx {
		0 => IndexMode::Content,
		1 => IndexMode::Shallow,
		2 => IndexMode::Deep,
		_ => IndexMode::Content,
	};

	println!();

	Ok(LocationAddInput {
		path: sd_path,
		name,
		mode,
	})
}
