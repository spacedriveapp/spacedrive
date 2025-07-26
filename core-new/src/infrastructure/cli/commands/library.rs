//! Library management commands
//!
//! This module handles CLI commands for managing libraries:
//! - Creating new libraries
//! - Listing existing libraries
//! - Switching between libraries
//! - Getting current library info

use crate::infrastructure::cli::daemon::{DaemonClient, DaemonCommand, DaemonResponse};
use crate::infrastructure::cli::output::messages::LibraryInfo as OutputLibraryInfo;
use crate::infrastructure::cli::output::{CliOutput, Message};
use clap::Subcommand;
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
		#[arg(long)]
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
	mut output: CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
	let mut client = DaemonClient::new_with_instance(instance_name.clone());

	match cmd {
		LibraryCommands::Create { name, path } => {
			output.info(&format!("Creating library '{}'...", name))?;

			match client
				.send_command(DaemonCommand::CreateLibrary {
					name: name.clone(),
					path,
				})
				.await
			{
				Ok(DaemonResponse::LibraryCreated { id, name, path }) => {
					output.print(Message::LibraryCreated { name, id, path })?;
				}
				Ok(DaemonResponse::Error(e)) => {
					output.error(Message::Error(format!("Failed to create library: {}", e)))?;
				}
				Err(e) => {
					output.error(Message::Error(format!(
						"Failed to communicate with daemon: {}",
						e
					)))?;
				}
				_ => {
					output.error(Message::Error(
						"Unexpected response from daemon".to_string(),
					))?;
				}
			}
		}

		LibraryCommands::Open { path } => {
			output.info(&format!("Opening library at {}...", path.display()))?;
			output.error(Message::Error(
				"Open command not yet implemented".to_string(),
			))?;
			output.info("Use 'spacedrive library create' to create a new library")?;
		}

		LibraryCommands::List { detailed } => {
			match client.send_command(DaemonCommand::ListLibraries).await {
				Ok(DaemonResponse::Libraries(libraries)) => {
					if libraries.is_empty() {
						output.print(Message::NoLibrariesFound)?;
					} else {
						let output_libs: Vec<OutputLibraryInfo> = libraries
							.into_iter()
							.map(|lib| OutputLibraryInfo {
								id: lib.id,
								name: lib.name,
								path: lib.path,
							})
							.collect();

						if detailed
							|| matches!(
								output.format(),
								crate::infrastructure::cli::output::OutputFormat::Json
							) {
							output.print(Message::LibraryList {
								libraries: output_libs,
							})?;
						} else {
							// For non-detailed human output, use a table
							let mut table = Table::new();
							table.set_header(vec!["ID", "Name", "Path"]);

							for lib in output_libs {
								table.add_row(vec![
									lib.id.to_string(),
									lib.name,
									lib.path.display().to_string(),
								]);
							}

							output.section().table(table).render()?;
						}
					}
				}
				Ok(DaemonResponse::Error(e)) => {
					output.error(Message::Error(format!("Failed to list libraries: {}", e)))?;
				}
				Err(e) => {
					output.error(Message::Error(format!(
						"Failed to communicate with daemon: {}",
						e
					)))?;
				}
				_ => {
					output.error(Message::Error(
						"Unexpected response from daemon".to_string(),
					))?;
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
					output.success("Switched library successfully")?;
				}
				Ok(DaemonResponse::Error(e)) => {
					output.error(Message::Error(format!("Failed to switch library: {}", e)))?;
				}
				Err(e) => {
					output.error(Message::Error(format!(
						"Failed to communicate with daemon: {}",
						e
					)))?;
				}
				_ => {
					output.error(Message::Error(
						"Unexpected response from daemon".to_string(),
					))?;
				}
			}
		}

		LibraryCommands::Current => {
			match client.send_command(DaemonCommand::GetCurrentLibrary).await {
				Ok(DaemonResponse::CurrentLibrary(lib_opt)) => {
					let library = lib_opt.map(|lib| OutputLibraryInfo {
						id: lib.id,
						name: lib.name,
						path: lib.path,
					});
					output.print(Message::CurrentLibrary { library })?;
				}
				Ok(DaemonResponse::Error(e)) => {
					output.error(Message::Error(format!("Error: {}", e)))?;
				}
				Err(e) => {
					output.error(Message::Error(format!(
						"Failed to communicate with daemon: {}",
						e
					)))?;
				}
				_ => {
					output.error(Message::Error(
						"Unexpected response from daemon".to_string(),
					))?;
				}
			}
		}

		LibraryCommands::Close => {
			output.info("Closing current library...")?;
			output.error(Message::Error(
				"Close command not yet implemented".to_string(),
			))?;
			output.info("This command will be available in a future update")?;
		}

		LibraryCommands::Delete { id, yes } => {
			if !yes {
				use dialoguer::Confirm;
				let confirm = Confirm::new()
					.with_prompt(format!("Are you sure you want to delete library '{}'?", id))
					.default(false)
					.interact()?;

				if !confirm {
					output.info("Operation cancelled")?;
					return Ok(());
				}
			}

			output.info(&format!("Deleting library {}...", id))?;
			output.error(Message::Error(
				"Delete command not yet implemented".to_string(),
			))?;
			output.info("This command will be available in a future update")?;
		}
	}

	Ok(())
}
