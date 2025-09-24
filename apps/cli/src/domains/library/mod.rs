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

use self::args::*;

#[derive(Subcommand, Debug)]
pub enum LibraryCmd {
    /// Create a new library
    Create(LibraryCreateArgs),
    /// List libraries
    List,
    /// Show detailed information about a library
    Info(LibraryInfoArgs),
    /// Delete a library
    Delete(LibraryDeleteArgs),
}

pub async fn run(ctx: &Context, cmd: LibraryCmd) -> Result<()> {
    match cmd {
        LibraryCmd::Create(args) => {
            let input: LibraryCreateInput = args.into();
            let out: LibraryCreateOutput = execute_action!(ctx, input);
            print_output!(ctx, &out, |o: &LibraryCreateOutput| {
                println!(
                    "Created library {} with ID {} at {}",
                    o.name, o.library_id, o.path.display()
                );
            });
        }
        LibraryCmd::List => {
            let out: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = execute_query!(ctx, sd_core::ops::libraries::list::query::ListLibrariesInput { include_stats: false });
            print_output!(ctx, &out, |libs: &Vec<sd_core::ops::libraries::list::output::LibraryInfo>| {
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
                println!("Created: {}", info.created_at.format("%Y-%m-%d %H:%M:%S UTC"));
                println!("Updated: {}", info.updated_at.format("%Y-%m-%d %H:%M:%S UTC"));
                println!();
                println!("Settings");
                println!("--------");
                println!("Generate thumbnails: {}", info.settings.generate_thumbnails);
                println!("Thumbnail quality: {}", info.settings.thumbnail_quality);
                println!("AI tagging enabled: {}", info.settings.enable_ai_tagging);
                println!("Sync enabled: {}", info.settings.sync_enabled);
                println!("Encryption enabled: {}", info.settings.encryption_enabled);
                println!("Auto track system volumes: {}", info.settings.auto_track_system_volumes);
                println!("Auto track external volumes: {}", info.settings.auto_track_external_volumes);
                println!("Max file size: {}",
                    info.settings.max_file_size
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
                    println!("Last indexed: {}", last_indexed.format("%Y-%m-%d %H:%M:%S UTC"));
                } else {
                    println!("Last indexed: Never");
                }
                println!("Stats updated: {}", info.statistics.updated_at.format("%Y-%m-%d %H:%M:%S UTC"));
            });
        }
        LibraryCmd::Delete(args) => {
            let msg = if args.delete_data {
                format!("This will delete library {} and ALL its data. Continue?", args.library_id)
            } else {
                format!("This will remove library {} from Spacedrive (data will remain). Continue?", args.library_id)
            };
            confirm_or_abort(&msg, args.yes)?;
            let input: sd_core::ops::libraries::delete::input::LibraryDeleteInput = args.into();
            let out: LibraryDeleteOutput = execute_action!(ctx, input);
            print_output!(ctx, &out, |o: &LibraryDeleteOutput| {
                println!("Deleted library {}", o.library_id);
            });
        }
    }
    Ok(())
}
