mod args;

use anyhow::Result;
use clap::Subcommand;

use crate::util::prelude::*;

use crate::context::Context;
use sd_core::ops::libraries::{
    create::output::LibraryCreateOutput,
    list::query::ListLibrariesQuery,
    session::set_current::SetCurrentLibraryOutput,
};

use self::args::*;

#[derive(Subcommand, Debug)]
pub enum LibraryCmd {
    /// Create a new library
    Create(LibraryCreateArgs),
    /// Switch to a different library
    Switch(LibrarySwitchArgs),
    /// List libraries
    List,
}

pub async fn run(ctx: &Context, cmd: LibraryCmd) -> Result<()> {
    match cmd {
        LibraryCmd::Create(args) => {
            let out: LibraryCreateOutput = execute_action!(ctx, args.into());
            print_output!(ctx, &out, |o: &LibraryCreateOutput| {
                println!(
                    "Created library {} with ID {} at {}",
                    o.name, o.library_id, o.path.display()
                );
            });
        }
        LibraryCmd::Switch(args) => {
            let out: SetCurrentLibraryOutput = execute_action!(ctx, args.into());
            print_output!(ctx, &out, |o: &SetCurrentLibraryOutput| {
                if o.success {
                    println!("Switched to library {}", args.id);
                } else {
                    println!("Failed to switch to library {}", args.id);
                }
            });
        }
        LibraryCmd::List => {
            let out: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = execute_query!(ctx, ListLibrariesQuery::basic());
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
    }
    Ok(())
}
