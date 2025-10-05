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
	input::LibrarySyncSetupInput,
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
				println!("Thumbnails: {}", info.statistics.thumbnail_count);
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
							println!("  Entries: {}", lib.statistics.total_entries);
							println!("  Locations: {}", lib.statistics.total_locations);
							println!("  Devices: {}", lib.statistics.device_count);
							if lib.statistics.total_size_bytes > 0 {
								println!("  Size: {} bytes", lib.statistics.total_size_bytes);
							}
						}
					}
				});
			}
			SyncSetupCmd::Setup(args) => {
				let input = args.to_input(ctx)?;
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
