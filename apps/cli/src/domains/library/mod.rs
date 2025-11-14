mod args;

use anyhow::Result;
use clap::Subcommand;

use crate::util::prelude::*;

use crate::context::Context;
use sd_core::ops::libraries::{
	create::{input::LibraryCreateInput, output::LibraryCreateOutput},
	delete::output::LibraryDeleteOutput,
	info::{output::LibraryInfoOutput, query::LibraryInfoQuery},
	list::query::ListLibrariesQuery,
};
use sd_core::ops::network::sync_setup::{
	discovery::{output::DiscoverRemoteLibrariesOutput, query::DiscoverRemoteLibrariesInput},
	input::{LibrarySyncAction, LibrarySyncSetupInput},
	output::LibrarySyncSetupOutput,
};

use self::args::*;

#[derive(Subcommand, Debug)]
pub enum LibraryCmd {
	/// Create a new library
	Create(LibraryCreateArgs),
	/// List libraries
	List,
	/// Show detailed information about a library
	Info(LibraryInfoArgs),
	/// Switch to a library
	Switch(LibrarySwitchArgs),
	/// Delete a library
	Delete(LibraryDeleteArgs),
	/// Library sync setup commands
	#[command(subcommand)]
	SyncSetup(SyncSetupCmd),
}

pub async fn run(ctx: &Context, cmd: LibraryCmd) -> Result<()> {
	match cmd {
		LibraryCmd::Create(args) => {
			let mut ctx = ctx.clone(); // Clone to allow mutation
			let input: LibraryCreateInput = args.into();
			let out: LibraryCreateOutput = execute_core_action!(ctx, input);

			// Automatically switch to the newly created library
			ctx.set_library_id(out.library_id)?;

			print_output!(ctx, &out, |o: &LibraryCreateOutput| {
				println!(
					"Created library {} with ID {} at {}",
					o.name,
					o.library_id,
					o.path.display()
				);
				println!("Switched to library {}", o.library_id);
			});
		}
		LibraryCmd::List => {
			let out: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = execute_core_query!(
				ctx,
				sd_core::ops::libraries::list::query::ListLibrariesInput {
					include_stats: false
				}
			);
			print_output!(ctx, &out, |libs: &Vec<
				sd_core::ops::libraries::list::output::LibraryInfo,
			>| {
				if libs.is_empty() {
					println!("No libraries found");
					return;
				}
				for l in libs {
					println!("- {} {}", l.id, l.path.display());
				}
			});
		}
		LibraryCmd::Info(args) => {
			// Get current library ID from CLI context if not specified
			let current_library_id = ctx.library_id;
			let input = args.to_input(current_library_id)?;
			let out: LibraryInfoOutput = execute_query!(ctx, input);
			print_output!(ctx, &out, |info: &LibraryInfoOutput| {
				println!("Library Information");
				println!("==================");
				println!("ID: {}", info.id);
				println!("Name: {}", info.name);
				if let Some(desc) = &info.description {
					println!("Description: {}", desc);
				}
				println!("Path: {}", info.path.display());
				println!(
					"Created: {}",
					info.created_at.format("%Y-%m-%d %H:%M:%S UTC")
				);
				println!(
					"Updated: {}",
					info.updated_at.format("%Y-%m-%d %H:%M:%S UTC")
				);
				println!();
				println!("Settings");
				println!("--------");
				println!("Generate thumbnails: {}", info.settings.generate_thumbnails);
				println!("Thumbnail quality: {}", info.settings.thumbnail_quality);
				println!("AI tagging enabled: {}", info.settings.enable_ai_tagging);
				println!("Sync enabled: {}", info.settings.sync_enabled);
				println!("Encryption enabled: {}", info.settings.encryption_enabled);
				println!(
					"Auto track system volumes: {}",
					info.settings.auto_track_system_volumes
				);
				println!(
					"Auto track external volumes: {}",
					info.settings.auto_track_external_volumes
				);
				println!(
					"Max file size: {}",
					info.settings
						.max_file_size
						.map(|size| format!("{} bytes", size))
						.unwrap_or_else(|| "No limit".to_string())
				);
				println!();
				println!("Statistics");
				println!("----------");
				println!("Total files: {}", info.statistics.total_files);
				println!("Total size: {} bytes", info.statistics.total_size);
				println!("Locations: {}", info.statistics.location_count);
				println!("Tags: {}", info.statistics.tag_count);
				println!("Devices: {}", info.statistics.device_count);
				println!("Total capacity: {} bytes", info.statistics.total_capacity);
				println!(
					"Available capacity: {} bytes",
					info.statistics.available_capacity
				);
				println!("Thumbnails: {}", info.statistics.thumbnail_count);
				println!("Database size: {} bytes", info.statistics.database_size);
				if let Some(last_indexed) = info.statistics.last_indexed {
					println!(
						"Last indexed: {}",
						last_indexed.format("%Y-%m-%d %H:%M:%S UTC")
					);
				} else {
					println!("Last indexed: Never");
				}
				println!(
					"Stats updated: {}",
					info.statistics.updated_at.format("%Y-%m-%d %H:%M:%S UTC")
				);
			});
		}
		LibraryCmd::Switch(args) => {
			let mut ctx = ctx.clone(); // Clone to allow mutation
			if let Some(library_id) = args.library_id {
				ctx.switch_to_library(library_id)?;
				println!("Switched to library {}", library_id);
			} else if let Some(name) = args.name {
				ctx.switch_to_library_named(&name).await?;
				println!("Switched to library '{}'", name);
			} else {
				// Show current library
				if let Some(library_id) = ctx.get_current_library_id() {
					if let Some(info) = ctx.get_current_library_info().await? {
						println!("Current library: {} ({})", info.name, library_id);
					} else {
						println!("Current library: {}", library_id);
					}
				} else {
					println!("No library selected");
				}
			}
		}
		LibraryCmd::Delete(args) => {
			let msg = if args.delete_data {
				format!(
					"This will delete library {} and ALL its data. Continue?",
					args.library_id
				)
			} else {
				format!(
					"This will remove library {} from Spacedrive (data will remain). Continue?",
					args.library_id
				)
			};
			confirm_or_abort(&msg, args.yes)?;
			let input: sd_core::ops::libraries::delete::input::LibraryDeleteInput = args.into();
			let out: LibraryDeleteOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &LibraryDeleteOutput| {
				println!("Deleted library {}", o.library_id);
			});
		}
		LibraryCmd::SyncSetup(cmd) => match cmd {
			SyncSetupCmd::Discover(args) => {
				let input: DiscoverRemoteLibrariesInput = args.into();
				let out: DiscoverRemoteLibrariesOutput = execute_core_query!(ctx, input);
				print_output!(ctx, &out, |o: &DiscoverRemoteLibrariesOutput| {
					println!("Device: {} ({})", o.device_name, o.device_id);
					println!("Online: {}", o.is_online);
					println!();
					if o.libraries.is_empty() {
						println!("No libraries found on remote device");
					} else {
						println!("Remote Libraries ({}):", o.libraries.len());
						println!("─────────────────────────────────────────");
						for lib in &o.libraries {
							println!();
							println!("  Name: {}", lib.name);
							println!("  ID: {}", lib.id);
							if let Some(desc) = &lib.description {
								println!("  Description: {}", desc);
							}
							println!("  Created: {}", lib.created_at.format("%Y-%m-%d %H:%M:%S"));
							println!("  Files: {}", lib.statistics.total_files);
							println!("  Locations: {}", lib.statistics.location_count);
							println!("  Thumbnails: {}", lib.statistics.thumbnail_count);
							if lib.statistics.total_size > 0 {
								println!("  Size: {} bytes", lib.statistics.total_size);
							}
						}
					}
				});
			}
			SyncSetupCmd::Setup(args) => {
				let input = if args.is_interactive() {
					run_interactive_sync_setup(ctx).await?
				} else {
					args.to_input(ctx)?
				};

				let out: LibrarySyncSetupOutput = execute_core_action!(ctx, input);
				print_output!(ctx, &out, |o: &LibrarySyncSetupOutput| {
					if o.success {
						println!("✓ Library sync setup successful");
						println!("  Local library: {}", o.local_library_id);
						if let Some(remote) = o.remote_library_id {
							println!("  Remote library: {}", remote);
						}
						println!("  {}", o.message);
					} else {
						println!("✗ Library sync setup failed");
						println!("  {}", o.message);
					}
				});
			}
		},
	}
	Ok(())
}

async fn run_interactive_sync_setup(ctx: &Context) -> Result<LibrarySyncSetupInput> {
	use crate::util::confirm::{select, text};
	use sd_core::ops::network::devices::{output::ListPairedDevicesOutput, query::ListPairedDevicesInput};

	println!("\n=== Library Sync Setup ===\n");

	// Get local device ID from config
	let config_path = ctx.data_dir.join("device.json");
	if !config_path.exists() {
		anyhow::bail!("Device config not found. Please run the daemon first to initialize device config.");
	}
	let config_data = std::fs::read_to_string(&config_path)?;
	let device_config: sd_core::device::DeviceConfig = serde_json::from_str(&config_data)?;
	let local_device_id = device_config.id;

	// Step 1: Select local library
	let libraries: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = execute_core_query!(
		ctx,
		sd_core::ops::libraries::list::query::ListLibrariesInput {
			include_stats: false
		}
	);

	if libraries.is_empty() {
		anyhow::bail!("No libraries found. Create a library first with:\n  sd library create <name>");
	}

	let library_choices: Vec<String> = libraries
		.iter()
		.map(|lib| format!("{} ({})", lib.name, lib.id))
		.collect();

	let library_idx = select("Select local library to sync", &library_choices)?;
	let local_library_id = libraries[library_idx].id;

	println!("\n✓ Selected local library: {}\n", libraries[library_idx].name);

	// Step 2: Select remote device from paired devices
	let paired_devices: ListPairedDevicesOutput = execute_core_query!(
		ctx,
		ListPairedDevicesInput {
			connected_only: false
		}
	);

	if paired_devices.devices.is_empty() {
		anyhow::bail!(
			"No paired devices found.\n\
			Pair a device first with:\n\
			  sd network pair generate  # on this device\n\
			  sd network pair join <code>  # on the other device"
		);
	}

	let device_choices: Vec<String> = paired_devices
		.devices
		.iter()
		.map(|d| {
			let status = if d.is_connected { "connected" } else { "paired" };
			format!("{} - {} ({})", d.name, d.os_version, status)
		})
		.collect();

	let device_idx = select("Select remote device to sync with", &device_choices)?;
	let remote_device = &paired_devices.devices[device_idx];
	let remote_device_id = remote_device.id;

	println!("\n✓ Selected remote device: {}\n", remote_device.name);

	// Step 3: Discover remote libraries
	println!("Discovering libraries on remote device...\n");

	let discovery_input = DiscoverRemoteLibrariesInput {
		device_id: remote_device_id,
	};
	let discovery_out: DiscoverRemoteLibrariesOutput = execute_core_query!(ctx, discovery_input);

	if !discovery_out.is_online {
		anyhow::bail!("Remote device {} is not online", remote_device.name);
	}

	// Step 4: Select sync action
	let action_idx = select(
		"Select sync action",
		&[
			"Share my library to remote device (create shared library from local)".to_string(),
			"Join remote library (use their existing library)".to_string(),
			"Merge libraries (combine two libraries) [NOT YET IMPLEMENTED]".to_string(),
		],
	)?;

	let action = match action_idx {
		0 => {
			// Share local library
			let name = libraries[library_idx].name.clone();

			println!(
				"\n✓ Will share library '{}' to remote device '{}'\n",
				name, remote_device.name
			);

			(
				LibrarySyncAction::ShareLocalLibrary { library_name: name },
				None,
			)
		}
		1 => {
			// Join remote library
			if discovery_out.libraries.is_empty() {
				anyhow::bail!("No libraries found on remote device. Use 'Share my library' instead.");
			}

			let remote_lib_choices: Vec<String> = discovery_out
				.libraries
				.iter()
				.map(|lib| {
					format!(
						"{} ({} entries, {} locations)",
						lib.name, lib.statistics.total_files, lib.statistics.location_count
					)
				})
				.collect();

			let remote_lib_idx = select("Select remote library to join", &remote_lib_choices)?;
			let remote_library = &discovery_out.libraries[remote_lib_idx];

			println!(
				"\n✓ Will join remote library: {}\n",
				remote_library.name
			);

			(
				LibrarySyncAction::JoinRemoteLibrary {
					remote_library_id: remote_library.id,
					remote_library_name: remote_library.name.clone(),
				},
				Some(remote_library.id),
			)
		}
		2 => {
			anyhow::bail!("Library merging is not yet implemented");
		}
		_ => unreachable!(),
	};

	// Leader device is always local for now (deprecated concept but still in input struct)
	let leader_device_id = local_device_id;

	Ok(LibrarySyncSetupInput {
		local_device_id,
		remote_device_id,
		local_library_id,
		remote_library_id: action.1,
		action: action.0,
		leader_device_id,
	})
}
