//! Library management commands
//!
//! This module handles CLI commands for managing libraries:
//! - Creating new libraries
//! - Listing existing libraries
//! - Switching between libraries
//! - Getting current library info

use crate::infrastructure::cli::daemon::{DaemonClient, DaemonCommand, DaemonResponse};
use clap::Subcommand;
use colored::Colorize;
use comfy_table::Table;
use std::path::PathBuf;

#[derive(Subcommand, Clone, Debug)]
pub enum LibraryCommands {
    /// Create a new library
    Create {
        /// Library name
        name: String,
        /// Path where to create the library
        #[arg(short, long)]
        path: Option<PathBuf>,
    },

    /// Open and switch to a library
    Open {
        /// Path to the library
        path: PathBuf,
    },

    /// Switch to a different library
    Switch {
        /// Library ID or name
        identifier: String,
    },

    /// List all libraries
    List {
        /// Show detailed information
        #[arg(short, long)]
        detailed: bool,
    },

    /// Show current library info
    Current,

    /// Close the current library
    Close,

    /// Delete a library
    Delete {
        /// Library ID to delete
        id: String,
        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
}

pub async fn handle_library_command(
    cmd: LibraryCommands,
    instance_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = DaemonClient::new_with_instance(instance_name.clone());

    match cmd {
        LibraryCommands::Create { name, path } => {
            println!("📚 Creating library '{}'...", name.bright_cyan());

            match client
                .send_command(DaemonCommand::CreateLibrary {
                    name: name.clone(),
                    path,
                })
                .await
            {
                Ok(DaemonResponse::LibraryCreated { id, name, path }) => {
                    println!("✅ Library created successfully!");
                    println!("   ID: {}", id.to_string().bright_yellow());
                    println!("   Path: {}", path.display().to_string().bright_blue());
                    println!("   Status: {}", "Active".bright_green());
                }
                Ok(DaemonResponse::Error(e)) => {
                    println!("❌ Failed to create library: {}", e);
                }
                Err(e) => {
                    println!("❌ Failed to communicate with daemon: {}", e);
                }
                _ => {
                    println!("❌ Unexpected response from daemon");
                }
            }
        }

        LibraryCommands::Open { path } => {
            println!("📚 Opening library at {}...", path.display());
            println!("❌ Open command not yet implemented");
            println!("   Use 'spacedrive library create' to create a new library");
        }

        LibraryCommands::List { detailed } => {
            match client
                .send_command(DaemonCommand::ListLibraries)
                .await
            {
                Ok(DaemonResponse::Libraries(libraries)) => {
                    if libraries.is_empty() {
                        println!("📭 No libraries found. Create one with: spacedrive library create <name>");
                    } else {
                        let mut table = Table::new();
                        table.set_header(vec!["ID", "Name", "Path"]);

                        for lib in libraries {
                            table.add_row(vec![
                                lib.id.to_string(),
                                lib.name,
                                lib.path.display().to_string(),
                            ]);
                        }

                        println!("{}", table);
                    }
                }
                Ok(DaemonResponse::Error(e)) => {
                    println!("❌ Failed to list libraries: {}", e);
                }
                Err(e) => {
                    println!("❌ Failed to communicate with daemon: {}", e);
                }
                _ => {
                    println!("❌ Unexpected response from daemon");
                }
            }
        }

        LibraryCommands::Switch { identifier } => {
            match client
                .send_command(DaemonCommand::SwitchLibrary {
                    id: identifier.parse()?,
                })
                .await
            {
                Ok(DaemonResponse::Ok) => {
                    println!("✅ Switched library successfully");
                }
                Ok(DaemonResponse::Error(e)) => {
                    println!("❌ Failed to switch library: {}", e);
                }
                Err(e) => {
                    println!("❌ Failed to communicate with daemon: {}", e);
                }
                _ => {
                    println!("❌ Unexpected response from daemon");
                }
            }
        }

        LibraryCommands::Current => {
            match client
                .send_command(DaemonCommand::GetCurrentLibrary)
                .await
            {
                Ok(DaemonResponse::CurrentLibrary(Some(lib))) => {
                    println!("📚 Current library: {}", lib.name.bright_cyan());
                    println!("   ID: {}", lib.id.to_string().bright_yellow());
                    println!("   Path: {}", lib.path.display().to_string().bright_blue());
                }
                Ok(DaemonResponse::CurrentLibrary(None)) => {
                    println!("⚠️  No current library selected");
                }
                Ok(DaemonResponse::Error(e)) => {
                    println!("❌ Error: {}", e);
                }
                Err(e) => {
                    println!("❌ Failed to communicate with daemon: {}", e);
                }
                _ => {
                    println!("❌ Unexpected response from daemon");
                }
            }
        }

        LibraryCommands::Close => {
            println!("📚 Closing current library...");
            println!("❌ Close command not yet implemented");
            println!("   This command will be available in a future update");
        }

        LibraryCommands::Delete { id, yes } => {
            if !yes {
                use dialoguer::Confirm;
                let confirm = Confirm::new()
                    .with_prompt(format!("Are you sure you want to delete library '{}'?", id))
                    .default(false)
                    .interact()?;
                
                if !confirm {
                    println!("Operation cancelled");
                    return Ok(());
                }
            }
            
            println!("🗑️  Deleting library {}...", id.bright_yellow());
            println!("❌ Delete command not yet implemented");
            println!("   This command will be available in a future update");
        }
    }

    Ok(())
}