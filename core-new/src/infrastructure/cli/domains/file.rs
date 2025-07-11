//! File operations commands
//!
//! This module handles CLI commands for file operations:
//! - Copying files using the action system
//! - Indexing operations with enhanced scope options
//! - Legacy scanning operations

use crate::infrastructure::cli::adapters::FileCopyCliArgs;
use crate::infrastructure::cli::daemon::{DaemonClient, DaemonCommand, DaemonResponse};
use crate::infrastructure::cli::output::{CliOutput, Message};
use clap::{Subcommand, ValueEnum};
use std::path::PathBuf;

// Re-export from the commands module for consistency
#[derive(Clone, Debug, ValueEnum)]
pub enum CliIndexMode {
    /// Only metadata (fast)
    Shallow,
    /// Metadata + content hashing
    Content,
    /// Full analysis including media metadata
    Deep,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum CliIndexScope {
    /// Full directory tree (default)
    Full,
    /// Only direct children
    Shallow,
    /// Custom depth
    Limited,
}

#[derive(Subcommand, Clone, Debug)]
pub enum FileCommands {
    /// Copy files using the action system
    Copy(FileCopyCliArgs),

    /// Enhanced indexing with scope and persistence options
    #[command(subcommand)]
    Index(IndexCommands),

    /// Start a traditional indexing job (legacy)
    Scan {
        /// Path to index
        path: PathBuf,

        /// Indexing mode
        #[arg(short, long, value_enum, default_value = "content")]
        mode: CliIndexMode,

        /// Monitor the job in real-time
        #[arg(short = 'w', long)]
        watch: bool,
    },
}

/// Enhanced indexing commands
#[derive(Subcommand, Clone, Debug)]
pub enum IndexCommands {
    /// Quick scan of a directory (metadata only, current scope)
    QuickScan {
        /// Path to scan
        path: PathBuf,
        /// Scope: shallow or full
        #[arg(short, long, value_enum, default_value = "shallow")]
        scope: CliIndexScope,
        /// Run ephemerally (no database writes)
        #[arg(short, long)]
        ephemeral: bool,
    },

    /// Browse external paths without adding to managed locations
    Browse {
        /// Path to browse
        path: PathBuf,
        /// Scope: shallow or full
        #[arg(short, long, value_enum, default_value = "shallow")]
        scope: CliIndexScope,
        /// Enable content analysis
        #[arg(short, long)]
        content: bool,
    },

    /// Index a specific path
    Path {
        /// Path to index
        path: PathBuf,
        /// Indexing mode
        #[arg(short, long, value_enum, default_value = "content")]
        mode: CliIndexMode,
        /// Indexing scope
        #[arg(short, long, value_enum, default_value = "full")]
        scope: CliIndexScope,
        /// Maximum depth (for limited scope)
        #[arg(short, long)]
        depth: Option<u32>,
        /// Create location if path doesn't exist in any
        #[arg(short, long)]
        create_location: bool,
        /// Monitor the job in real-time
        #[arg(short = 'w', long)]
        watch: bool,
    },

    /// Re-index all locations
    All {
        /// Force re-indexing even if up-to-date
        #[arg(short, long)]
        force: bool,
        /// Monitor jobs in real-time
        #[arg(short = 'w', long)]
        watch: bool,
    },

    /// Index a specific location
    Location {
        /// Location ID or name
        location: String,
        /// Force re-indexing
        #[arg(short, long)]
        force: bool,
        /// Monitor the job in real-time
        #[arg(short = 'w', long)]
        watch: bool,
    },
}

pub async fn handle_file_command(
    cmd: FileCommands,
    instance_name: Option<String>,
    mut output: CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = DaemonClient::new_with_instance(instance_name.clone());

    match cmd {
        FileCommands::Copy(args) => {
            handle_copy_command(args, &mut client, &mut output).await
        }
        FileCommands::Index(cmd) => {
            handle_index_command(cmd, &mut client, &mut output).await
        }
        FileCommands::Scan { path, mode, watch } => {
            handle_scan_command(path, mode, watch, &mut client, &mut output).await
        }
    }
}

async fn handle_copy_command(
    args: FileCopyCliArgs,
    client: &mut DaemonClient,
    output: &mut CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
    // Convert CLI args to daemon command format
    let input = match args.validate_and_convert() {
        Ok(input) => input,
        Err(e) => {
            output.error(Message::Error(format!("Invalid copy operation: {}", e)))?;
            return Ok(());
        }
    };

    output.info(&input.summary())?;

    // Send copy command to daemon
    match client
        .send_command(DaemonCommand::Copy {
            sources: input.sources.clone(),
            destination: input.destination.clone(),
            overwrite: input.overwrite,
            verify: input.verify_checksum,
            preserve_timestamps: input.preserve_timestamps,
            move_files: input.move_files,
        })
        .await
    {
        Ok(DaemonResponse::CopyStarted {
            job_id,
            sources_count,
        }) => {
            output.success("Copy operation started successfully!")?;
            
            let mut section = output.section()
                .item("Job ID", &job_id.to_string())
                .item("Sources", &format!("{} file(s)", sources_count))
                .item("Destination", &input.destination.display().to_string());
            
            if input.overwrite {
                section = section.item("Mode", "Overwrite existing files");
            }
            if input.verify_checksum {
                section = section.item("Verification", "Enabled");
            }
            if input.move_files {
                section = section.item("Type", "Move (delete source after copy)");
            }
            
            section.empty_line()
                .help()
                    .item("Monitor progress with: spacedrive job monitor")
                .render()?;
        }
        Ok(DaemonResponse::Ok) => {
            output.success("Copy operation completed successfully!")?;
        }
        Ok(DaemonResponse::Error(e)) => {
            output.error(Message::Error(format!("Failed to copy files: {}", e)))?;
        }
        Err(e) => {
            output.error(Message::Error(format!("Failed to communicate with daemon: {}", e)))?;
        }
        _ => {
            output.error(Message::Error("Unexpected response from daemon".to_string()))?;
        }
    }

    Ok(())
}

async fn handle_index_command(
    cmd: IndexCommands,
    client: &mut DaemonClient,
    output: &mut CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        IndexCommands::QuickScan { path, scope, ephemeral } => {
            output.info(&format!("Quick scanning {}...", path.display()))?;
            if ephemeral {
                output.info("Running in ephemeral mode (no database writes)")?;
            }
            
            let scope_str = match scope {
                CliIndexScope::Full => "full",
                CliIndexScope::Shallow => "shallow",
                CliIndexScope::Limited => "limited",
            };
            
            match client
                .send_command(DaemonCommand::QuickScan {
                    path: path.clone(),
                    scope: scope_str.to_string(),
                    ephemeral,
                })
                .await
            {
                Ok(DaemonResponse::Ok) => {
                    output.success("Quick scan completed successfully")?;
                }
                Ok(DaemonResponse::Error(e)) => {
                    output.error(Message::Error(format!("Quick scan failed: {}", e)))?;
                }
                Err(e) => {
                    output.error(Message::Error(format!("Failed to communicate with daemon: {}", e)))?;
                }
                _ => {
                    output.error(Message::Error("Unexpected response from daemon".to_string()))?;
                }
            }
        }
        
        IndexCommands::Browse { path, scope, content } => {
            output.info(&format!("Browsing {}...", path.display()))?;
            if content {
                output.info("Content analysis enabled")?;
            }
            
            let scope_str = match scope {
                CliIndexScope::Full => "full",
                CliIndexScope::Shallow => "shallow",
                CliIndexScope::Limited => "limited",
            };
            
            match client
                .send_command(DaemonCommand::Browse {
                    path: path.clone(),
                    scope: scope_str.to_string(),
                    content,
                })
                .await
            {
                Ok(DaemonResponse::Ok) => {
                    output.success("Browse completed successfully")?;
                }
                Ok(DaemonResponse::Error(e)) => {
                    output.error(Message::Error(format!("Browse failed: {}", e)))?;
                }
                Err(e) => {
                    output.error(Message::Error(format!("Failed to communicate with daemon: {}", e)))?;
                }
                _ => {
                    output.error(Message::Error("Unexpected response from daemon".to_string()))?;
                }
            }
        }
        
        IndexCommands::Path { path, mode, scope, depth, create_location, watch } => {
            output.info(&format!("Indexing path {}...", path.display()))?;
            
            let mode_str = match mode {
                CliIndexMode::Shallow => "shallow",
                CliIndexMode::Content => "content",
                CliIndexMode::Deep => "deep",
            };
            
            let scope_str = match scope {
                CliIndexScope::Full => "full",
                CliIndexScope::Shallow => "shallow",
                CliIndexScope::Limited => "limited",
            };
            
            match client
                .send_command(DaemonCommand::IndexPath {
                    path: path.clone(),
                    mode: mode_str.to_string(),
                    scope: scope_str.to_string(),
                    depth,
                    create_location,
                })
                .await
            {
                Ok(DaemonResponse::Ok) => {
                    output.success("Path indexing started successfully")?;
                    if watch {
                        output.info("Use 'spacedrive job monitor' to track progress")?;
                    }
                }
                Ok(DaemonResponse::Error(e)) => {
                    output.error(Message::Error(format!("Path indexing failed: {}", e)))?;
                }
                Err(e) => {
                    output.error(Message::Error(format!("Failed to communicate with daemon: {}", e)))?;
                }
                _ => {
                    output.error(Message::Error("Unexpected response from daemon".to_string()))?;
                }
            }
        }
        IndexCommands::All { force, watch } => {
            output.info("Re-indexing all locations...")?;
            
            match client
                .send_command(DaemonCommand::IndexAll { force })
                .await
            {
                Ok(DaemonResponse::Ok) => {
                    output.success("Re-indexing of all locations started successfully")?;
                    if watch {
                        output.info("Use 'spacedrive job monitor' to track progress")?;
                    }
                }
                Ok(DaemonResponse::Error(e)) => {
                    output.error(Message::Error(format!("Re-indexing failed: {}", e)))?;
                }
                Err(e) => {
                    output.error(Message::Error(format!("Failed to communicate with daemon: {}", e)))?;
                }
                _ => {
                    output.error(Message::Error("Unexpected response from daemon".to_string()))?;
                }
            }
        }
        IndexCommands::Location { location, force, watch } => {
            output.info(&format!("Indexing location {}...", location))?;
            
            match client
                .send_command(DaemonCommand::IndexLocation {
                    location: location.clone(),
                    force,
                })
                .await
            {
                Ok(DaemonResponse::Ok) => {
                    output.success("Location indexing started successfully")?;
                    if watch {
                        output.info("Use 'spacedrive job monitor' to track progress")?;
                    }
                }
                Ok(DaemonResponse::Error(e)) => {
                    output.error(Message::Error(format!("Location indexing failed: {}", e)))?;
                }
                Err(e) => {
                    output.error(Message::Error(format!("Failed to communicate with daemon: {}", e)))?;
                }
                _ => {
                    output.error(Message::Error("Unexpected response from daemon".to_string()))?;
                }
            }
        }
    }
    Ok(())
}

async fn handle_scan_command(
    path: PathBuf,
    mode: CliIndexMode,
    watch: bool,
    client: &mut DaemonClient,
    output: &mut CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
    output.info(&format!("Scanning {}...", path.display()))?;
    
    let mode_str = match mode {
        CliIndexMode::Shallow => "shallow",
        CliIndexMode::Content => "content",
        CliIndexMode::Deep => "deep",
    };
    
    // Use IndexPath command with recursive scope for traditional scan
    match client
        .send_command(DaemonCommand::IndexPath {
            path: path.clone(),
            mode: mode_str.to_string(),
            scope: "full".to_string(),
            depth: None,
            create_location: false,
        })
        .await
    {
        Ok(DaemonResponse::Ok) => {
            output.success("Scan started successfully")?;
            if watch {
                output.info("Use 'spacedrive job monitor' to track progress")?;
            }
        }
        Ok(DaemonResponse::Error(e)) => {
            output.error(Message::Error(format!("Scan failed: {}", e)))?;
        }
        Err(e) => {
            output.error(Message::Error(format!("Failed to communicate with daemon: {}", e)))?;
        }
        _ => {
            output.error(Message::Error("Unexpected response from daemon".to_string()))?;
        }
    }
    
    Ok(())
}