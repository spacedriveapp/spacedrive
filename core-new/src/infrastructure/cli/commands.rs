use crate::{
    Core,
    library::Library,
    location::{create_location, LocationCreateArgs},
    infrastructure::{
        database::entities,
        jobs::types::JobStatus,
    },
};
use super::state::CliState;
use clap::{Subcommand, ValueEnum};
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;
use sea_orm::{EntityTrait, QueryFilter, ColumnTrait, PaginatorTrait};
use colored::*;
use comfy_table::{Table, Cell, presets::UTF8_FULL};

#[derive(Clone, Debug, ValueEnum)]
pub enum IndexMode {
    /// Only metadata (fast)
    Shallow,
    /// Metadata + content hashing
    Content,
    /// Full analysis including media metadata
    Deep,
}

impl From<IndexMode> for crate::location::IndexMode {
    fn from(mode: IndexMode) -> Self {
        match mode {
            IndexMode::Shallow => crate::location::IndexMode::Shallow,
            IndexMode::Content => crate::location::IndexMode::Content,
            IndexMode::Deep => crate::location::IndexMode::Deep,
        }
    }
}

#[derive(Subcommand, Clone)]
pub enum LibraryCommands {
    /// Create a new library
    Create {
        /// Name of the library
        name: String,
        /// Path where to create the library
        #[arg(short, long)]
        path: Option<PathBuf>,
    },
    
    /// List all libraries
    List,
    
    /// Open and switch to a library
    Open {
        /// Path to the library
        path: PathBuf,
    },
    
    /// Switch to a library by name or ID
    Switch {
        /// Library name or UUID
        identifier: String,
    },
    
    /// Show current library info
    Current,
    
    /// Close the current library
    Close,
}

#[derive(Subcommand, Clone)]
pub enum LocationCommands {
    /// Add a new location to the current library
    Add {
        /// Path to add as a location
        path: PathBuf,
        /// Name for the location
        #[arg(short, long)]
        name: Option<String>,
        /// Indexing mode
        #[arg(short, long, value_enum, default_value = "content")]
        mode: IndexMode,
    },
    
    /// List all locations in the current library
    List,
    
    /// Remove a location
    Remove {
        /// Location ID or path
        identifier: String,
    },
    
    /// Rescan a location
    Rescan {
        /// Location ID or path
        identifier: String,
        /// Force full rescan (ignore change detection)
        #[arg(short, long)]
        force: bool,
    },
    
    /// Show location details
    Info {
        /// Location ID or path
        identifier: String,
    },
}

#[derive(Subcommand, Clone)]
pub enum JobCommands {
    /// List all jobs
    List {
        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,
        /// Show only recent jobs
        #[arg(short, long)]
        recent: bool,
    },
    
    /// Show job details
    Info {
        /// Job ID
        id: Uuid,
    },
    
    /// Monitor jobs in real-time
    Monitor {
        /// Optional job ID to monitor a specific job
        #[arg(short, long)]
        job_id: Option<String>,
    },
    
    /// Pause a running job
    Pause {
        /// Job ID
        id: Uuid,
    },
    
    /// Resume a paused job
    Resume {
        /// Job ID
        id: Uuid,
    },
    
    /// Cancel a job
    Cancel {
        /// Job ID
        id: Uuid,
    },
}

pub async fn handle_library_command(
    cmd: LibraryCommands,
    core: &Core,
    state: &mut CliState,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        LibraryCommands::Create { name, path } => {
            println!("📚 Creating library '{}'...", name.bright_cyan());
            
            let library = core.libraries.create_library(&name, path).await?;
            let lib_id = library.id();
            let lib_path = library.path().to_path_buf();
            
            state.set_current_library(lib_id, lib_path.clone());
            
            println!("✅ Library created successfully!");
            println!("   ID: {}", lib_id.to_string().bright_yellow());
            println!("   Path: {}", lib_path.display().to_string().bright_blue());
            println!("   Status: {}", "Active".bright_green());
        }
        
        LibraryCommands::List => {
            let libraries = core.libraries.list().await;
            
            if libraries.is_empty() {
                println!("📭 No libraries found. Create one with: {}", "spacedrive library create <name>".bright_cyan());
                return Ok(());
            }
            
            let mut table = Table::new();
            table.load_preset(UTF8_FULL)
                .set_header(vec!["Status", "Name", "ID", "Path"]);
            
            for library in libraries {
                let id = library.id();
                let name = library.name().await;
                let path = library.path();
                let is_current = state.current_library_id == Some(id);
                
                let status = if is_current {
                    "●".bright_green().to_string()
                } else {
                    "○".normal().to_string()
                };
                
                table.add_row(vec![
                    Cell::new(status),
                    Cell::new(name),
                    Cell::new(id.to_string()),
                    Cell::new(path.display()),
                ]);
            }
            
            println!("{}", table);
        }
        
        LibraryCommands::Open { path } => {
            println!("📂 Opening library at {}...", path.display().to_string().bright_blue());
            
            let library = core.libraries.open_library(&path).await?;
            let lib_id = library.id();
            
            state.set_current_library(lib_id, path.clone());
            
            println!("✅ Library opened successfully!");
            println!("   Name: {}", library.name().await.bright_cyan());
            println!("   ID: {}", lib_id.to_string().bright_yellow());
        }
        
        LibraryCommands::Switch { identifier } => {
            let libraries = core.libraries.list().await;
            
            let mut found_library = None;
            for lib in libraries {
                let lib_name = lib.name().await;
                if lib.id().to_string().starts_with(&identifier) || lib_name == identifier {
                    found_library = Some((lib, lib_name));
                    break;
                }
            }
            
            match found_library {
                Some((lib, lib_name)) => {
                    let lib_id = lib.id();
                    let lib_path = lib.path().to_path_buf();
                    state.set_current_library(lib_id, lib_path);
                    
                    println!("✅ Switched to library: {}", lib_name.bright_cyan());
                }
                None => {
                    println!("❌ Library not found: {}", identifier.bright_red());
                }
            }
        }
        
        LibraryCommands::Current => {
            if let Some(lib_id) = &state.current_library_id {
                let libraries = core.libraries.list().await;
                if let Some(library) = libraries.into_iter().find(|lib| lib.id() == *lib_id) {
                    println!("📚 Current library: {}", library.name().await.bright_cyan());
                    println!("   ID: {}", lib_id.to_string().bright_yellow());
                    println!("   Path: {}", library.path().display().to_string().bright_blue());
                } else {
                    println!("⚠️  Current library no longer exists");
                    state.current_library_id = None;
                }
            } else {
                println!("📭 No library selected. Use: {}", "spacedrive library open <path>".bright_cyan());
            }
        }
        
        LibraryCommands::Close => {
            if let Some(lib_id) = state.current_library_id {
                core.libraries.close_library(lib_id).await?;
                state.current_library_id = None;
                println!("✅ Library closed");
            } else {
                println!("📭 No library is currently open");
            }
        }
    }
    
    Ok(())
}

pub async fn handle_location_command(
    cmd: LocationCommands,
    core: &Core,
    state: &mut CliState,
) -> Result<(), Box<dyn std::error::Error>> {
    // Ensure we have a current library
    let library = get_current_library(core, state).await?;
    
    match cmd {
        LocationCommands::Add { path, name, mode } => {
            println!("📍 Adding location: {}...", path.display().to_string().bright_blue());
            
            // Get device from database
            let db = library.db();
            let device = core.device.to_device()?;
            
            let device_record = entities::device::Entity::find()
                .filter(entities::device::Column::Uuid.eq(device.id))
                .one(db.conn())
                .await?
                .ok_or("Device not registered in database")?;
            
            // Create location
            let location_args = LocationCreateArgs {
                path: path.clone(),
                name: name.clone(),
                index_mode: mode.into(),
            };
            
            let location_id = create_location(
                library,
                &core.events,
                location_args,
                device_record.id,
            ).await?;
            
            println!("✅ Location added successfully!");
            println!("   ID: {}", location_id.to_string().bright_yellow());
            println!("   Name: {}", name.unwrap_or_else(|| path.file_name().unwrap().to_string_lossy().to_string()).bright_cyan());
            println!("   Path: {}", path.display().to_string().bright_blue());
            println!("   Status: {} (job dispatched)", "Indexing".bright_yellow());
            
            println!("\n💡 Tip: Monitor indexing progress with: {}", "spacedrive job monitor".bright_cyan());
        }
        
        LocationCommands::List => {
            let db = library.db();
            let locations = entities::location::Entity::find()
                .all(db.conn())
                .await?;
            
            if locations.is_empty() {
                println!("📭 No locations found. Add one with: {}", "spacedrive location add <path>".bright_cyan());
                return Ok(());
            }
            
            let mut table = Table::new();
            table.load_preset(UTF8_FULL)
                .set_header(vec!["ID", "Name", "Path", "Mode", "Status", "Files", "Size"]);
            
            for location in locations {
                let status_color = match location.scan_state.as_str() {
                    "pending" => "Pending".bright_yellow(),
                    "scanning" => "Scanning".bright_blue(),
                    "complete" => "Complete".bright_green(),
                    "error" => "Error".bright_red(),
                    "paused" => "Paused".bright_magenta(),
                    _ => "Unknown".normal(),
                };
                
                let size_str = format_bytes(location.total_byte_size as u64);
                
                table.add_row(vec![
                    Cell::new(location.id),
                    Cell::new(location.name.unwrap_or_default()),
                    Cell::new(location.path),
                    Cell::new(location.index_mode),
                    Cell::new(status_color),
                    Cell::new(location.total_file_count),
                    Cell::new(size_str),
                ]);
            }
            
            println!("{}", table);
        }
        
        LocationCommands::Remove { identifier } => {
            println!("🗑️  Removing location {}...", identifier.bright_red());
            // TODO: Implement location removal
            println!("⚠️  Location removal not yet implemented");
        }
        
        LocationCommands::Rescan { identifier, force } => {
            println!("🔄 Rescanning location {}...", identifier.bright_blue());
            if force {
                println!("   Mode: {} (ignoring change detection)", "Full scan".bright_yellow());
            }
            // TODO: Implement rescan
            println!("⚠️  Location rescan not yet implemented");
        }
        
        LocationCommands::Info { identifier } => {
            let db = library.db();
            
            // Try to find by ID first, then by path
            let location = if let Ok(id) = identifier.parse::<i32>() {
                entities::location::Entity::find_by_id(id)
                    .one(db.conn())
                    .await?
            } else {
                entities::location::Entity::find()
                    .filter(entities::location::Column::Path.contains(&identifier))
                    .one(db.conn())
                    .await?
            };
            
            match location {
                Some(loc) => {
                    println!("📍 Location Details");
                    println!("   ID: {}", loc.id.to_string().bright_yellow());
                    println!("   Name: {}", loc.name.unwrap_or_default().bright_cyan());
                    println!("   Path: {}", loc.path.bright_blue());
                    println!("   Mode: {}", loc.index_mode.bright_magenta());
                    println!("   Status: {}", match loc.scan_state.as_str() {
                        "complete" => loc.scan_state.bright_green(),
                        "scanning" => loc.scan_state.bright_blue(),
                        "error" => loc.scan_state.bright_red(),
                        _ => loc.scan_state.normal(),
                    });
                    println!("   Files: {}", loc.total_file_count.to_string().bright_white());
                    println!("   Size: {}", format_bytes(loc.total_byte_size as u64).bright_white());
                    
                    if let Some(last_scan) = loc.last_scan_at {
                        println!("   Last scan: {}", last_scan.to_string().bright_white());
                    }
                    
                    if let Some(error) = loc.error_message {
                        println!("   Error: {}", error.bright_red());
                    }
                }
                None => {
                    println!("❌ Location not found: {}", identifier.bright_red());
                }
            }
        }
    }
    
    Ok(())
}

pub async fn handle_job_command(
    cmd: JobCommands,
    core: &Core,
    state: &mut CliState,
) -> Result<(), Box<dyn std::error::Error>> {
    let library = get_current_library(core, state).await?;
    
    match cmd {
        JobCommands::List { status, recent } => {
            let status_filter = status.and_then(|s| match s.to_lowercase().as_str() {
                "running" => Some(JobStatus::Running),
                "completed" => Some(JobStatus::Completed),
                "failed" => Some(JobStatus::Failed),
                "paused" => Some(JobStatus::Paused),
                "cancelled" => Some(JobStatus::Cancelled),
                _ => None,
            });
            
            let jobs = library.jobs().list_jobs(status_filter).await?;
            
            if jobs.is_empty() {
                println!("📭 No jobs found");
                return Ok(());
            }
            
            let mut table = Table::new();
            table.load_preset(UTF8_FULL)
                .set_header(vec!["ID", "Type", "Status", "Progress", "Started", "Duration"]);
            
            let display_jobs = if recent {
                jobs.into_iter().take(10).collect()
            } else {
                jobs
            };
            
            for job in display_jobs {
                let status_color = match job.status {
                    JobStatus::Running => "Running".bright_blue(),
                    JobStatus::Completed => "Completed".bright_green(),
                    JobStatus::Failed => "Failed".bright_red(),
                    JobStatus::Paused => "Paused".bright_yellow(),
                    JobStatus::Cancelled => "Cancelled".bright_magenta(),
                    _ => "Unknown".normal(),
                };
                
                let progress = format!("{:.1}%", job.progress * 100.0);
                let duration = if let Some(completed) = job.completed_at {
                    let duration = completed - job.started_at;
                    format!("{:.1}s", duration.num_seconds())
                } else {
                    let duration = chrono::Utc::now() - job.started_at;
                    format!("{:.1}s", duration.num_seconds())
                };
                
                table.add_row(vec![
                    Cell::new(job.id.to_string().chars().take(8).collect::<String>()),
                    Cell::new(job.name),
                    Cell::new(status_color),
                    Cell::new(progress),
                    Cell::new(job.started_at.format("%H:%M:%S")),
                    Cell::new(duration),
                ]);
            }
            
            println!("{}", table);
            
            if recent {
                println!("\n💡 Showing recent jobs. Use without {} to see all", "--recent".bright_cyan());
            }
        }
        
        JobCommands::Info { id } => {
            if let Some(job) = library.jobs().get_job_info(id).await? {
                println!("💼 Job Details");
                println!("   ID: {}", job.id.to_string().bright_yellow());
                println!("   Type: {}", job.name.bright_cyan());
                println!("   Status: {}", match job.status {
                    JobStatus::Running => "Running".bright_blue(),
                    JobStatus::Completed => "Completed".bright_green(),
                    JobStatus::Failed => "Failed".bright_red(),
                    JobStatus::Paused => "Paused".bright_yellow(),
                    JobStatus::Cancelled => "Cancelled".bright_magenta(),
                    _ => "Unknown".normal(),
                });
                println!("   Progress: {:.1}%", job.progress * 100.0);
                println!("   Started: {}", job.started_at.format("%Y-%m-%d %H:%M:%S"));
                
                if let Some(completed) = job.completed_at {
                    println!("   Completed: {}", completed.format("%Y-%m-%d %H:%M:%S"));
                    let duration = completed - job.started_at;
                    println!("   Duration: {:.1}s", duration.num_seconds());
                }
                
                if let Some(error) = job.error_message {
                    println!("   Error: {}", error.bright_red());
                }
            } else {
                println!("❌ Job not found: {}", id.to_string().bright_red());
            }
        }
        
        JobCommands::Monitor { job_id } => {
            if let Some(id) = job_id {
                // Monitor specific job
                if let Ok(uuid) = id.parse::<Uuid>() {
                    println!("📊 Monitoring job {}...", id.chars().take(8).collect::<String>().bright_yellow());
                    // TODO: Implement single job monitoring
                    println!("⚠️  Single job monitoring not yet implemented. Showing all jobs:");
                }
            }
            super::monitor::run_monitor(core).await?;
        }
        
        JobCommands::Pause { id } => {
            println!("⏸️  Pausing job {}...", id.to_string().bright_yellow());
            // TODO: Implement job pause
            println!("⚠️  Job pause not yet implemented");
        }
        
        JobCommands::Resume { id } => {
            println!("▶️  Resuming job {}...", id.to_string().bright_blue());
            // TODO: Implement job resume
            println!("⚠️  Job resume not yet implemented");
        }
        
        JobCommands::Cancel { id } => {
            println!("❌ Cancelling job {}...", id.to_string().bright_red());
            // TODO: Implement job cancel
            println!("⚠️  Job cancel not yet implemented");
        }
    }
    
    Ok(())
}

pub async fn handle_index_command(
    path: PathBuf,
    mode: IndexMode,
    watch: bool,
    core: &Core,
    state: &mut CliState,
) -> Result<(), Box<dyn std::error::Error>> {
    let library = get_current_library(core, state).await?;
    
    println!("🔍 Starting indexing job...");
    println!("   Path: {}", path.display().to_string().bright_blue());
    println!("   Mode: {}", format!("{:?}", mode).bright_magenta());
    
    // Get device from database
    let db = library.db();
    let device = core.device.to_device()?;
    
    let device_record = entities::device::Entity::find()
        .filter(entities::device::Column::Uuid.eq(device.id))
        .one(db.conn())
        .await?
        .ok_or("Device not registered in database")?;
    
    // Create location and start indexing
    let location_args = LocationCreateArgs {
        path: path.clone(),
        name: Some(path.file_name().unwrap().to_string_lossy().to_string()),
        index_mode: mode.into(),
    };
    
    let location_id = create_location(
        library.clone(),
        &core.events,
        location_args,
        device_record.id,
    ).await?;
    
    println!("✅ Indexing job started!");
    println!("   Location ID: {}", location_id.to_string().bright_yellow());
    
    if watch {
        println!("\n📡 Monitoring job progress...\n");
        super::monitor::run_monitor(core).await?;
    } else {
        println!("\n💡 Monitor progress with: {}", "spacedrive job monitor".bright_cyan());
    }
    
    Ok(())
}

pub async fn handle_status_command(
    core: &Core,
    state: &CliState,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 {} Status", "Spacedrive CLI".bright_cyan());
    println!("═══════════════════════════════════════");
    
    // Device info
    let device = core.device.to_device()?;
    println!("\n📱 Device");
    println!("   ID: {}", device.id.to_string().bright_yellow());
    println!("   Name: {}", device.name.bright_white());
    println!("   OS: {} {}", device.os, device.hardware_model.as_deref().unwrap_or(""));
    
    // Current library
    println!("\n📚 Library");
    if let Some(lib_id) = &state.current_library_id {
        let libraries = core.libraries.list().await;
        if let Some(library) = libraries.into_iter().find(|lib| lib.id() == *lib_id) {
            println!("   Current: {}", library.name().await.bright_cyan());
            println!("   ID: {}", lib_id.to_string().bright_yellow());
            println!("   Path: {}", library.path().display().to_string().bright_blue());
            
            // Get stats
            let db = library.db();
            let entry_count = entities::entry::Entity::find()
                .count(db.conn())
                .await
                .unwrap_or(0);
            let location_count = entities::location::Entity::find()
                .count(db.conn())
                .await
                .unwrap_or(0);
            
            println!("   Entries: {}", entry_count.to_string().bright_white());
            println!("   Locations: {}", location_count.to_string().bright_white());
        } else {
            println!("   ⚠️  Current library no longer exists");
        }
    } else {
        println!("   📭 No library selected");
    }
    
    // System info
    println!("\n🖥️  System");
    println!("   Event subscribers: {}", core.events.subscriber_count());
    println!("   Libraries loaded: {}", core.libraries.list().await.len());
    
    Ok(())
}

async fn get_current_library(
    core: &Core,
    state: &CliState,
) -> Result<Arc<Library>, Box<dyn std::error::Error>> {
    if let Some(lib_id) = &state.current_library_id {
        let libraries = core.libraries.list().await;
        libraries
            .into_iter()
            .find(|lib| lib.id() == *lib_id)
            .ok_or_else(|| "Current library not found. Please select a library.".into())
    } else {
        Err("No library selected. Use 'spacedrive library open <path>' or 'spacedrive library create <name>'.".into())
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    format!("{:.2} {}", size, UNITS[unit_index])
}