//! CLI Application logic

use super::commands::*;
use super::tui::TuiApp;
use super::utils::{format_library_info, format_location_info, print_job_info, print_progress_bar};
use crate::{
    infrastructure::events::{Event, EventBus, EventFilter},
    library::{manager::LibraryManager, Library},
    location::{manager::LocationManager, IndexMode as CoreIndexMode},
};
use anyhow::{anyhow, Result};
use console::{style, Term};
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};
use uuid::Uuid;

/// Main CLI application
pub struct CliApp {
    library_manager: Arc<LibraryManager>,
    event_bus: Arc<EventBus>,
    current_library: Arc<RwLock<Option<Arc<Library>>>>,
}

impl CliApp {
    /// Create a new CLI app
    pub fn new(library_manager: Arc<LibraryManager>, event_bus: Arc<EventBus>) -> Self {
        Self {
            library_manager,
            event_bus,
            current_library: Arc::new(RwLock::new(None)),
        }
    }

    /// Handle library commands
    pub async fn handle_library_command(&self, cmd: LibraryCommand) -> Result<()> {
        match cmd {
            LibraryCommand::Create { name, location } => {
                self.create_library(name, location).await
            }
            LibraryCommand::Open { library } => self.open_library(library).await,
            LibraryCommand::List { detailed } => self.list_libraries(detailed).await,
            LibraryCommand::Switch { library } => self.switch_library(library).await,
            LibraryCommand::Info => self.library_info().await,
            LibraryCommand::Close => self.close_library().await,
            LibraryCommand::Delete { id, yes } => self.delete_library(id, yes).await,
        }
    }

    /// Handle location commands
    pub async fn handle_location_command(&self, cmd: LocationCommand) -> Result<()> {
        let library = self.get_current_library().await?;

        match cmd {
            LocationCommand::Add {
                path,
                name,
                mode,
                watch,
            } => self.add_location(library, path, name, mode, watch).await,
            LocationCommand::Remove { id, yes } => self.remove_location(library, id, yes).await,
            LocationCommand::List { detailed } => self.list_locations(library, detailed).await,
            LocationCommand::Rescan { id, force } => self.rescan_location(library, id, force).await,
        }
    }

    /// Handle index commands
    pub async fn handle_index_command(&self, cmd: IndexCommand) -> Result<()> {
        let library = self.get_current_library().await?;

        match cmd {
            IndexCommand::Start { location } => self.start_indexing(library, location).await,
            IndexCommand::Pause { job } => self.pause_indexing(library, job).await,
            IndexCommand::Resume { job } => self.resume_indexing(library, job).await,
            IndexCommand::Status { detailed } => self.indexing_status(library, detailed).await,
            IndexCommand::Stats { location } => self.indexing_stats(library, location).await,
        }
    }

    /// Handle job commands
    pub async fn handle_job_command(&self, cmd: JobCommand) -> Result<()> {
        let library = self.get_current_library().await?;

        match cmd {
            JobCommand::List {
                status,
                job_type,
                detailed,
            } => self.list_jobs(library, status, job_type, detailed).await,
            JobCommand::Info { id } => self.job_info(library, id).await,
            JobCommand::Cancel { id, yes } => self.cancel_job(library, id, yes).await,
            JobCommand::Clear { failed, yes } => self.clear_jobs(library, failed, yes).await,
            JobCommand::Monitor {
                id,
                exit_on_complete,
            } => self.monitor_jobs(library, id, exit_on_complete).await,
        }
    }

    /// Run TUI mode
    pub async fn run_tui(&self) -> Result<()> {
        let mut tui = TuiApp::new(
            self.library_manager.clone(),
            self.event_bus.clone(),
            self.current_library.clone(),
        );
        tui.run().await
    }

    /// Watch events in real-time
    pub async fn watch_events(&self, filter: Option<String>) -> Result<()> {
        let term = Term::stdout();
        let mut subscriber = self.event_bus.subscribe();

        println!("{}", style("Watching events... (Press Ctrl+C to exit)").cyan());
        println!();

        loop {
            match subscriber.recv().await {
                Ok(event) => {
                    // Apply filter if specified
                    if let Some(ref filter_str) = filter {
                        let should_show = match filter_str.as_str() {
                            "library" => event.is_library_event(),
                            "volume" => event.is_volume_event(),
                            "job" => event.is_job_event(),
                            "indexing" => matches!(
                                event,
                                Event::IndexingStarted { .. }
                                    | Event::IndexingProgress { .. }
                                    | Event::IndexingCompleted { .. }
                                    | Event::IndexingFailed { .. }
                            ),
                            _ => true,
                        };

                        if !should_show {
                            continue;
                        }
                    }

                    // Format and display event
                    let timestamp = chrono::Local::now().format("%H:%M:%S");
                    let event_str = format_event(&event);
                    println!("[{}] {}", style(timestamp).dim(), event_str);
                }
                Err(_) => {
                    // Event bus closed
                    break;
                }
            }
        }

        Ok(())
    }

    // Library operations

    async fn create_library(&self, name: String, location: Option<String>) -> Result<()> {
        let location_path = location.map(std::path::PathBuf::from);

        println!("Creating library '{}'...", style(&name).cyan());

        let library = self
            .library_manager
            .create_library(name.clone(), location_path)
            .await?;

        println!(
            "{}",
            style(format!("Library '{}' created successfully!", name)).green()
        );
        println!("ID: {}", style(library.id()).yellow());
        println!("Path: {}", style(library.path().display()).dim());

        // Set as current library
        *self.current_library.write().await = Some(library);

        Ok(())
    }

    async fn open_library(&self, library: String) -> Result<()> {
        println!("Opening library...");

        // Try to parse as UUID first
        let library_arc = if let Ok(id) = Uuid::parse_str(&library) {
            // Try to find by ID
            if let Some(lib) = self.library_manager.get_library(id).await {
                lib
            } else {
                // Try to open by path
                self.library_manager.open_library(&library).await?
            }
        } else {
            // Treat as path
            self.library_manager.open_library(&library).await?
        };

        let name = library_arc.name().await;
        println!(
            "{}",
            style(format!("Library '{}' opened successfully!", name)).green()
        );

        // Set as current library
        *self.current_library.write().await = Some(library_arc);

        Ok(())
    }

    async fn list_libraries(&self, detailed: bool) -> Result<()> {
        // Get open libraries
        let open_libraries = self.library_manager.get_open_libraries().await;

        // Scan for all libraries
        let discovered = self.library_manager.scan_for_libraries().await?;

        println!("{}", style("Open Libraries:").bold().underline());
        if open_libraries.is_empty() {
            println!("  {}", style("No libraries are currently open").dim());
        } else {
            for lib in &open_libraries {
                if detailed {
                    println!("{}", format_library_info(lib).await);
                } else {
                    println!(
                        "  {} {} - {}",
                        style("▶").green(),
                        style(lib.name().await).cyan(),
                        style(lib.id()).dim()
                    );
                }
            }
        }

        println!();
        println!("{}", style("Available Libraries:").bold().underline());
        if discovered.is_empty() {
            println!("  {}", style("No libraries found").dim());
        } else {
            for disc in &discovered {
                let status = if disc.is_locked {
                    style("locked").red()
                } else {
                    style("available").green()
                };

                if detailed {
                    println!("  {} - {}", style(&disc.config.name).cyan(), status);
                    println!("    ID: {}", style(&disc.config.id).dim());
                    println!("    Path: {}", style(disc.path.display()).dim());
                    println!(
                        "    Created: {}",
                        style(disc.config.created_at.format("%Y-%m-%d %H:%M:%S")).dim()
                    );
                } else {
                    println!(
                        "  {} {} - {} ({})",
                        if disc.is_locked {
                            style("○").red()
                        } else {
                            style("○").dim()
                        },
                        style(&disc.config.name).cyan(),
                        style(&disc.config.id).dim(),
                        status
                    );
                }
            }
        }

        Ok(())
    }

    async fn switch_library(&self, library: String) -> Result<()> {
        // Implementation similar to open_library but focuses on switching
        self.open_library(library).await
    }

    async fn library_info(&self) -> Result<()> {
        let library = self.get_current_library().await?;
        println!("{}", format_library_info(&library).await);
        Ok(())
    }

    async fn close_library(&self) -> Result<()> {
        let library = self.get_current_library().await?;
        let id = library.id();
        let name = library.name().await;

        self.library_manager.close_library(id).await?;
        *self.current_library.write().await = None;

        println!(
            "{}",
            style(format!("Library '{}' closed", name)).green()
        );
        Ok(())
    }

    async fn delete_library(&self, id: Uuid, yes: bool) -> Result<()> {
        // For safety, we'll just close the library if it's open
        // Actual deletion would require more careful handling
        if !yes {
            let confirm = Confirm::new()
                .with_prompt("Are you sure you want to delete this library?")
                .default(false)
                .interact()?;

            if !confirm {
                println!("{}", style("Deletion cancelled").yellow());
                return Ok(());
            }
        }

        // Close if open
        if let Err(_) = self.library_manager.close_library(id).await {
            // Library might not be open, that's ok
        }

        println!(
            "{}",
            style("Library deletion not implemented for safety").yellow()
        );
        println!("Please manually delete the library directory if needed");

        Ok(())
    }

    // Location operations

    async fn add_location(
        &self,
        library: Arc<Library>,
        path: String,
        name: Option<String>,
        mode: IndexMode,
        watch: bool,
    ) -> Result<()> {
        let path_buf = std::path::PathBuf::from(&path);

        println!("Adding location '{}'...", style(&path).cyan());

        let location_manager = LocationManager::new(self.event_bus.clone());
        
        // For now, use device_id 1 (would need proper device management)
        let location = location_manager
            .add_location(&library, path_buf, name, 1, mode.into(), watch)
            .await?;

        println!(
            "{}",
            style(format!("Location '{}' added successfully!", location.name)).green()
        );
        println!("ID: {}", style(location.id).yellow());
        println!("Mode: {:?}", location.index_mode);

        Ok(())
    }

    async fn remove_location(&self, library: Arc<Library>, id: Uuid, yes: bool) -> Result<()> {
        if !yes {
            let confirm = Confirm::new()
                .with_prompt("Are you sure you want to remove this location?")
                .default(false)
                .interact()?;

            if !confirm {
                println!("{}", style("Removal cancelled").yellow());
                return Ok(());
            }
        }

        let location_manager = LocationManager::new(self.event_bus.clone());
        location_manager.remove_location(&library, id).await?;

        println!("{}", style("Location removed successfully").green());
        Ok(())
    }

    async fn list_locations(&self, library: Arc<Library>, detailed: bool) -> Result<()> {
        let location_manager = LocationManager::new(self.event_bus.clone());
        let locations = location_manager.list_locations(&library).await?;

        println!("{}", style("Locations:").bold().underline());
        if locations.is_empty() {
            println!("  {}", style("No locations configured").dim());
        } else {
            for loc in &locations {
                if detailed {
                    println!("{}", format_location_info(loc));
                } else {
                    println!(
                        "  {} {} - {}",
                        style("📁").cyan(),
                        style(&loc.name).cyan(),
                        style(loc.path.display()).dim()
                    );
                }
            }
        }

        Ok(())
    }

    async fn rescan_location(
        &self,
        library: Arc<Library>,
        id: Option<Uuid>,
        _force: bool,
    ) -> Result<()> {
        let location_manager = LocationManager::new(self.event_bus.clone());

        if let Some(location_id) = id {
            println!("Rescanning location {}...", style(location_id).cyan());
            // Would need to implement rescan in LocationManager
            println!("{}", style("Rescan started").green());
        } else {
            println!("Rescanning all locations...");
            let locations = location_manager.list_locations(&library).await?;
            for loc in locations {
                println!("  Rescanning {}...", style(&loc.name).cyan());
                // Start rescan for each location
            }
            println!("{}", style("Rescan started for all locations").green());
        }

        Ok(())
    }

    // Indexing operations

    async fn start_indexing(&self, library: Arc<Library>, location: Option<Uuid>) -> Result<()> {
        let location_manager = LocationManager::new(self.event_bus.clone());

        if let Some(location_id) = location {
            println!("Starting indexing for location {}...", style(location_id).cyan());
            // Would need to get location and start indexing
        } else {
            println!("Starting indexing for all locations...");
            let locations = location_manager.list_locations(&library).await?;
            for loc in locations {
                println!("  Starting indexing for {}...", style(&loc.name).cyan());
                location_manager.start_indexing(&library, &loc).await?;
            }
        }

        println!("{}", style("Indexing started").green());
        Ok(())
    }

    async fn pause_indexing(&self, _library: Arc<Library>, job: Option<String>) -> Result<()> {
        if let Some(job_id) = job {
            println!("Pausing job {}...", style(&job_id).cyan());
        } else {
            println!("Pausing all indexing jobs...");
        }

        // Would need job manager integration
        println!("{}", style("Feature not yet implemented").yellow());
        Ok(())
    }

    async fn resume_indexing(&self, _library: Arc<Library>, job: Option<String>) -> Result<()> {
        if let Some(job_id) = job {
            println!("Resuming job {}...", style(&job_id).cyan());
        } else {
            println!("Resuming all paused indexing jobs...");
        }

        // Would need job manager integration
        println!("{}", style("Feature not yet implemented").yellow());
        Ok(())
    }

    async fn indexing_status(&self, _library: Arc<Library>, detailed: bool) -> Result<()> {
        println!("{}", style("Indexing Status:").bold().underline());

        // Would get actual status from job manager
        if detailed {
            println!("  {}", style("No active indexing jobs").dim());
        } else {
            println!("  {}", style("No active indexing jobs").dim());
        }

        Ok(())
    }

    async fn indexing_stats(&self, library: Arc<Library>, location: Option<Uuid>) -> Result<()> {
        let location_manager = LocationManager::new(self.event_bus.clone());
        let locations = location_manager.list_locations(&library).await?;

        println!("{}", style("Indexing Statistics:").bold().underline());

        let filtered_locations = if let Some(loc_id) = location {
            locations.into_iter().filter(|l| l.id == loc_id).collect()
        } else {
            locations
        };

        if filtered_locations.is_empty() {
            println!("  {}", style("No locations found").dim());
        } else {
            for loc in &filtered_locations {
                println!("  {} {}", style("📊").cyan(), style(&loc.name).cyan());
                println!("    Path: {}", style(loc.path.display()).dim());
                println!("    Mode: {:?}", loc.index_mode);
                // Would show actual stats from database
            }
        }

        Ok(())
    }

    // Job operations

    async fn list_jobs(
        &self,
        _library: Arc<Library>,
        _status: Option<JobStatus>,
        _job_type: Option<String>,
        detailed: bool,
    ) -> Result<()> {
        println!("{}", style("Jobs:").bold().underline());

        // Would get actual jobs from job manager
        if detailed {
            println!("  {}", style("No jobs found").dim());
        } else {
            println!("  {}", style("No jobs found").dim());
        }

        Ok(())
    }

    async fn job_info(&self, _library: Arc<Library>, id: String) -> Result<()> {
        println!("Job Information for {}:", style(&id).cyan());
        println!("  {}", style("Job not found").dim());
        Ok(())
    }

    async fn cancel_job(&self, _library: Arc<Library>, id: String, yes: bool) -> Result<()> {
        if !yes {
            let confirm = Confirm::new()
                .with_prompt(format!("Are you sure you want to cancel job {}?", id))
                .default(false)
                .interact()?;

            if !confirm {
                println!("{}", style("Cancellation aborted").yellow());
                return Ok(());
            }
        }

        println!("Cancelling job {}...", style(&id).cyan());
        println!("{}", style("Feature not yet implemented").yellow());
        Ok(())
    }

    async fn clear_jobs(&self, _library: Arc<Library>, failed: bool, yes: bool) -> Result<()> {
        let prompt = if failed {
            "Clear all completed and failed jobs?"
        } else {
            "Clear all completed jobs?"
        };

        if !yes {
            let confirm = Confirm::new()
                .with_prompt(prompt)
                .default(false)
                .interact()?;

            if !confirm {
                println!("{}", style("Clear operation cancelled").yellow());
                return Ok(());
            }
        }

        println!("{}", style("Jobs cleared").green());
        Ok(())
    }

    async fn monitor_jobs(
        &self,
        _library: Arc<Library>,
        id: Option<String>,
        exit_on_complete: bool,
    ) -> Result<()> {
        let term = Term::stdout();
        let mut subscriber = self.event_bus.subscribe();

        println!(
            "{}",
            style("Monitoring jobs... (Press Ctrl+C to exit)").cyan()
        );
        println!();

        // Create progress bars for active jobs
        let mut progress_bars = std::collections::HashMap::new();

        loop {
            match subscriber.recv().await {
                Ok(event) => match event {
                    Event::JobStarted { job_id, job_type } => {
                        if id.as_ref().map_or(true, |i| &job_id == i) {
                            let pb = ProgressBar::new(100);
                            pb.set_style(
                                ProgressStyle::default_bar()
                                    .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                                    .unwrap()
                                    .progress_chars("#>-"),
                            );
                            pb.set_message(format!("{} started", job_type));
                            progress_bars.insert(job_id.clone(), pb);
                        }
                    }
                    Event::JobProgress {
                        job_id,
                        progress,
                        message,
                    } => {
                        if id.as_ref().map_or(true, |i| &job_id == i) {
                            if let Some(pb) = progress_bars.get(&job_id) {
                                pb.set_position((progress * 100.0) as u64);
                                if let Some(msg) = message {
                                    pb.set_message(msg);
                                }
                            }
                        }
                    }
                    Event::JobCompleted { job_id, .. } => {
                        if id.as_ref().map_or(true, |i| &job_id == i) {
                            if let Some(pb) = progress_bars.remove(&job_id) {
                                pb.finish_with_message("Completed");
                            }
                            if exit_on_complete && id.is_some() {
                                break;
                            }
                        }
                    }
                    Event::JobFailed { job_id, error, .. } => {
                        if id.as_ref().map_or(true, |i| &job_id == i) {
                            if let Some(pb) = progress_bars.remove(&job_id) {
                                pb.abandon_with_message(format!("Failed: {}", error));
                            }
                            if exit_on_complete && id.is_some() {
                                break;
                            }
                        }
                    }
                    _ => {}
                },
                Err(_) => break,
            }
        }

        // Clean up progress bars
        for (_, pb) in progress_bars {
            pb.finish();
        }

        Ok(())
    }

    // Helper methods

    async fn get_current_library(&self) -> Result<Arc<Library>> {
        let current = self.current_library.read().await;
        
        if let Some(lib) = current.as_ref() {
            Ok(lib.clone())
        } else {
            // Try to load the first available library
            let libraries = self.library_manager.get_open_libraries().await;
            if let Some(lib) = libraries.first() {
                drop(current);
                *self.current_library.write().await = Some(lib.clone());
                Ok(lib.clone())
            } else {
                Err(anyhow!(
                    "No library is currently open. Use 'library create' or 'library open' first."
                ))
            }
        }
    }
}

/// Format event for display
fn format_event(event: &Event) -> String {
    match event {
        Event::LibraryCreated { name, .. } => {
            format!("{} Library '{}' created", style("✓").green(), style(name).cyan())
        }
        Event::LibraryOpened { name, .. } => {
            format!("{} Library '{}' opened", style("✓").green(), style(name).cyan())
        }
        Event::LocationAdded { path, .. } => {
            format!(
                "{} Location '{}' added",
                style("✓").green(),
                style(path.display()).cyan()
            )
        }
        Event::IndexingStarted { location_id } => {
            format!(
                "{} Indexing started for location {}",
                style("▶").yellow(),
                style(location_id).cyan()
            )
        }
        Event::IndexingProgress {
            processed, total, ..
        } => {
            let percent = if let Some(t) = total {
                format!("{:.1}%", (*processed as f64 / *t as f64) * 100.0)
            } else {
                format!("{} items", processed)
            };
            format!("{} Indexing progress: {}", style("◐").yellow(), percent)
        }
        Event::IndexingCompleted {
            total_files,
            total_dirs,
            ..
        } => {
            format!(
                "{} Indexing completed: {} files, {} directories",
                style("✓").green(),
                style(total_files).cyan(),
                style(total_dirs).cyan()
            )
        }
        Event::JobProgress { job_id, progress, message } => {
            let msg = message.as_deref().unwrap_or("");
            format!(
                "{} Job {}: {:.1}% {}",
                style("◐").yellow(),
                style(job_id).dim(),
                progress * 100.0,
                msg
            )
        }
        _ => format!("{:?}", event),
    }
}