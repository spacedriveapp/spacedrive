use crate::{
	infrastructure::events::Event, infrastructure::jobs::generic_progress::GenericProgress, Core,
};
use colored::*;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

pub async fn run_monitor(core: &Core) -> Result<(), Box<dyn std::error::Error>> {
	println!(
		"📡 {} - Press Ctrl+C to exit",
		"Spacedrive Job Monitor!".bright_cyan()
	);
	println!("═══════════════════════════════════════════");
	println!();

	// Subscribe to events
	let mut event_sub = core.events.subscribe();

	// Progress bars for jobs
	let multi_progress = MultiProgress::new();
	let mut job_bars: HashMap<Uuid, ProgressBar> = HashMap::new();

	// Create style for progress bars
	let style = ProgressStyle::with_template(
		"{spinner:.green} {prefix:.bold.cyan} [{bar:40.green/blue}] {percent}% | {msg}",
	)?
	.progress_chars("█▓▒░");

	// Enhanced style for jobs with generic progress
	let enhanced_style = ProgressStyle::with_template(
        "{spinner:.green} {prefix:.bold.cyan} [{bar:40.green/blue}] {percent}% | {msg}\n     {wide_msg}"
    )?
    .progress_chars("█▓▒░");

	println!("⏳ Waiting for job activity...\n");

	// Main event loop
	loop {
		tokio::select! {
			Ok(event) = event_sub.recv() => {
				match event {
					Event::JobProgress { job_id, job_type, progress, message, generic_progress } => {
						if let Ok(uuid) = job_id.parse::<Uuid>() {
							// Get or create progress bar for this job
							let pb = job_bars.entry(uuid).or_insert_with(|| {
								let bar = multi_progress.add(ProgressBar::new(100));
								bar.set_style(enhanced_style.clone());
								bar.set_prefix(format!("{} {}",
									job_type.bright_yellow(),
									job_id.chars().take(8).collect::<String>().bright_white()
								));
								bar
							});

							// Update progress
							pb.set_position((progress * 100.0) as u64);

							// Process enhanced progress data if available
							if let Some(gp_value) = generic_progress {
								if let Ok(gp) = serde_json::from_value::<GenericProgress>(gp_value) {
									// Build rich message from generic progress
									let phase_msg = format!("{}: {}",
										gp.phase.bright_blue(),
										gp.message
									);

									// Build detailed progress info
									let mut details = Vec::new();

									// Add completion info
									if gp.completion.total > 0 {
										details.push(format!("{}/{} items",
											gp.completion.completed,
											gp.completion.total
										));
									}

									// Add bytes info
									if let (Some(bytes), Some(total)) = (gp.completion.bytes_completed, gp.completion.total_bytes) {
										details.push(format!("{}/{}",
											format_bytes(bytes),
											format_bytes(total)
										));
									}

									// Add rate info
									if gp.performance.rate > 0.0 {
										details.push(format!("{:.1} items/s", gp.performance.rate));
									}

									// Add ETA
									if let Some(eta) = gp.performance.estimated_remaining {
										details.push(format!("ETA: {}", format_duration(eta)));
									}

									// Add error count if any
									if gp.performance.error_count > 0 {
										details.push(format!("{} errors", gp.performance.error_count).red().to_string());
									}

									// Set the detailed message
									if !details.is_empty() {
										pb.set_message(format!("{} • {}", phase_msg, details.join(" • ").bright_white()));
									}
								} else if let Some(msg) = message {
									pb.set_message(msg);
								}
							} else if let Some(msg) = message {
								pb.set_message(msg);
							}
						}
					}

					Event::JobCompleted { job_id, .. } => {
						if let Ok(uuid) = job_id.parse::<Uuid>() {
							if let Some(pb) = job_bars.get(&uuid) {
								pb.finish_with_message("✅ Completed".bright_green().to_string());
							}
						}
						println!("\n✅ Job {} completed!", job_id.chars().take(8).collect::<String>().bright_green());
					}

					Event::JobFailed { job_id, job_type: _, error } => {
						if let Ok(uuid) = job_id.parse::<Uuid>() {
							if let Some(pb) = job_bars.get(&uuid) {
								pb.abandon_with_message(format!("❌ Failed: {}", error).bright_red().to_string());
							}
						}
						println!("\n❌ Job {} failed: {}",
							job_id.chars().take(8).collect::<String>().bright_red(),
							error.bright_red()
						);
					}

					Event::IndexingStarted { location_id } => {
						println!("🔍 {} for location {}",
							"Indexing started".bright_blue(),
							location_id.to_string().bright_yellow()
						);
					}

					Event::IndexingCompleted { location_id, total_files, total_dirs } => {
						println!("✅ {} for location {}: {} files, {} directories",
							"Indexing completed".bright_green(),
							location_id.to_string().bright_yellow(),
							total_files.to_string().bright_white(),
							total_dirs.to_string().bright_white()
						);
					}

					Event::FilesIndexed { count, .. } => {
						// This is handled by job progress
					}

					_ => {
						// Log other events
						println!("📢 Event: {:?}", event);
					}
				}
			}

			_ = tokio::signal::ctrl_c() => {
				println!("\n\n👋 Exiting monitor...");
				break;
			}
		}
	}

	Ok(())
}

/// Monitor a specific indexing job with a nice progress display
pub async fn monitor_indexing_job(
	core: &Core,
	job_id: Uuid,
	location_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
	println!(
		"\n🔍 {} {}",
		"Indexing".bright_blue().bold(),
		location_path.bright_cyan()
	);

	// Subscribe to events
	let mut event_sub = core.events.subscribe();

	// Create progress bar with custom style
	let pb = ProgressBar::new(100);
	pb.set_style(
		ProgressStyle::with_template(
			"{spinner:.green} [{elapsed_precise}] [{bar:50.cyan/blue}] {percent:>3}% | {msg}",
		)?
		.progress_chars("█▓▒░")
		.tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
	);

	// Enable steady tick for spinner animation
	pb.enable_steady_tick(Duration::from_millis(100));

	// Track indexing stats
	let mut total_files = 0u64;
	let mut total_dirs = 0u64;
	let mut total_bytes = 0u64;
	let mut current_phase = String::from("Starting...");

	// Monitor events
	loop {
		tokio::select! {
			Ok(event) = event_sub.recv() => {
				match event {
					Event::JobProgress { job_id: event_job_id, progress, message, .. } => {
						if event_job_id == job_id.to_string() {
							// Update progress bar
							pb.set_position((progress * 100.0) as u64);

							// Display any message provided
							if let Some(msg) = &message {
								// Try to extract some basic info from the message
								if msg.contains("files") || msg.contains("dirs") {
									pb.set_message(msg.clone());
								} else {
									pb.set_message(format!("{} | {}", current_phase.bright_yellow(), msg));
								}
							}
						}
					}

					Event::IndexingProgress { location_id: _, processed, total } => {
						// Alternative progress update
						if let Some(total_val) = total {
							if total_val > 0 {
								let percent = (processed as f32 / total_val as f32 * 100.0) as u64;
								pb.set_position(percent);
							}
						}
					}

					Event::JobCompleted { job_id: event_job_id, .. } => {
						if event_job_id == job_id.to_string() {
							pb.finish_with_message(format!(
								"✅ Complete: {} files, {} dirs, {}",
								total_files.to_string().bright_green(),
								total_dirs.to_string().bright_green(),
								format_bytes(total_bytes).bright_green()
							));
							break;
						}
					}

					Event::JobFailed { job_id: event_job_id, error, .. } => {
						if event_job_id == job_id.to_string() {
							pb.abandon_with_message(format!("❌ Failed: {}", error.bright_red()));
							return Err(error.into());
						}
					}

					Event::IndexingCompleted { location_id: _, total_files: files, total_dirs: dirs } => {
						// Update final stats if available
						if files > 0 || dirs > 0 {
							total_files = files as u64;
							total_dirs = dirs as u64;
						}
					}

					_ => {}
				}
			}

			_ = tokio::time::sleep(Duration::from_secs(30)) => {
				// Timeout after 30 seconds of no events
				pb.abandon_with_message("⚠️  Timeout - no progress updates received");
				break;
			}
		}
	}

	println!(
		"\n✨ {} indexed successfully!",
		"Location".bright_green().bold()
	);

	Ok(())
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

fn format_duration(duration: Duration) -> String {
	let secs = duration.as_secs();
	if secs < 60 {
		format!("{}s", secs)
	} else if secs < 3600 {
		format!("{}m {}s", secs / 60, secs % 60)
	} else {
		format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
	}
}
