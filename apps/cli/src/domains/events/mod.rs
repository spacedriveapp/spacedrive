//! Events domain for monitoring event bus in real-time

mod args;

pub use args::*;

use crate::context::Context;
use anyhow::Result;
use chrono::{DateTime, Utc};
use sd_core::infra::daemon::types::EventFilter;
use sd_core::infra::event::Event;
use std::collections::HashSet;

/// Run events command
pub async fn run(ctx: &Context, cmd: EventsCmd) -> Result<()> {
	match cmd {
		EventsCmd::Monitor(args) => run_events_monitor(ctx, args).await,
	}
}

/// Monitor events in real-time
async fn run_events_monitor(ctx: &Context, args: EventsMonitorArgs) -> Result<()> {
	println!("Monitoring events - Press Ctrl+C to exit");

	// Parse event types filter
	let event_types_filter: Option<HashSet<String>> = args
		.event_type
		.as_ref()
		.map(|types| types.split(',').map(|s| s.trim().to_string()).collect());

	if let Some(ref types) = event_types_filter {
		println!(
			"Filtering by event types: {}",
			types.iter().cloned().collect::<Vec<_>>().join(", ")
		);
	}
	if let Some(ref lib_id) = args.library_id {
		println!("Filtering by library: {}", lib_id);
	}
	if let Some(ref job_id) = args.job_id {
		println!("Filtering by job: {}", job_id);
	}
	if let Some(ref device_id) = args.device_id {
		println!("Filtering by device: {}", device_id);
	}
	println!("═══════════════════════════════════════════════════════");

	// Create event filter for daemon
	let filter = EventFilter {
		library_id: args.library_id,
		job_id: args.job_id.clone(),
		device_id: args.device_id,
		resource_type: None,
		path_scope: None,
	};

	// Subscribe to all events (we'll filter by type client-side)
	let event_types = vec![];

	match ctx.core.subscribe_events(event_types, Some(filter)).await {
		Ok(mut event_stream) => {
			println!("Connected to event stream");
			println!();

			// Listen for events
			while let Some(event) = event_stream.recv().await {
				// Apply client-side event type filter
				if let Some(ref types_filter) = event_types_filter {
					let event_variant = event.variant_name();
					if !types_filter.contains(event_variant) {
						continue;
					}
				}

				// Display the event
				display_event(&event, &args);
			}
		}

		Err(e) => {
			println!("Failed to connect to event stream: {}", e);
			println!("Make sure the daemon is running");
		}
	}

	Ok(())
}

/// Display an event based on formatting preferences
fn display_event(event: &Event, args: &EventsMonitorArgs) {
	let timestamp = if args.timestamps {
		format!("[{}] ", Utc::now().format("%H:%M:%S%.3f"))
	} else {
		String::new()
	};

	let event_variant = event.variant_name();

	if args.verbose || args.pretty {
		// Verbose mode: show full JSON
		let json_str = if args.pretty {
			serde_json::to_string_pretty(&event).unwrap_or_else(|_| format!("{:?}", event))
		} else {
			serde_json::to_string(&event).unwrap_or_else(|_| format!("{:?}", event))
		};
		println!("{}{}: {}", timestamp, event_variant, json_str);
	} else {
		// Compact mode: show event type and key fields
		let summary = summarize_event(event);
		println!("{}{}: {}", timestamp, event_variant, summary);
	}
}

/// Create a human-readable summary of an event
fn summarize_event(event: &Event) -> String {
	match event {
		// Core lifecycle
		Event::CoreStarted => "Core started".to_string(),
		Event::CoreShutdown => "Core shutting down".to_string(),

		// Cache invalidation
		Event::Refresh => "Cache refresh triggered".to_string(),

		// Library events
		Event::LibraryCreated { name, id, .. } => {
			format!("Library '{}' created ({})", name, id)
		}
		Event::LibraryOpened { name, id, .. } => {
			format!("Library '{}' opened ({})", name, id)
		}
		Event::LibraryClosed { name, id } => {
			format!("Library '{}' closed ({})", name, id)
		}
		Event::LibraryDeleted {
			name,
			id,
			deleted_data,
		} => {
			format!(
				"Library '{}' deleted ({}, data: {})",
				name, id, deleted_data
			)
		}
		Event::LibraryStatisticsUpdated { library_id, .. } => {
			format!("Statistics updated for library {}", library_id)
		}

		// Entry events
		Event::EntryCreated {
			entry_id,
			library_id,
		} => {
			format!("Entry {} created in library {}", entry_id, library_id)
		}
		Event::EntryModified {
			entry_id,
			library_id,
		} => {
			format!("Entry {} modified in library {}", entry_id, library_id)
		}
		Event::EntryDeleted {
			entry_id,
			library_id,
		} => {
			format!("Entry {} deleted in library {}", entry_id, library_id)
		}
		Event::EntryMoved {
			entry_id,
			old_path,
			new_path,
			..
		} => {
			format!(
				"Entry {} moved from '{}' to '{}'",
				entry_id, old_path, new_path
			)
		}

		// Filesystem events
		Event::FsRawChange { library_id, kind } => {
			format!("Filesystem change in library {}: {:?}", library_id, kind)
		}

		// Volume events
		Event::VolumeAdded(vol) => {
			format!("Volume added: {} ({})", vol.name, vol.fingerprint.0)
		}
		Event::VolumeRemoved { fingerprint } => {
			format!("Volume removed: {}", fingerprint.0)
		}
		Event::VolumeUpdated { fingerprint, .. } => {
			format!("Volume updated: {}", fingerprint.0)
		}
		Event::VolumeSpeedTested {
			fingerprint,
			read_speed_mbps,
			write_speed_mbps,
		} => {
			format!(
				"Volume {} speed: read {}MB/s, write {}MB/s",
				fingerprint.0, read_speed_mbps, write_speed_mbps
			)
		}
		Event::VolumeMountChanged {
			fingerprint,
			is_mounted,
		} => {
			format!(
				"Volume {} mount changed: {}",
				fingerprint.0,
				if *is_mounted { "mounted" } else { "unmounted" }
			)
		}
		Event::VolumeError { fingerprint, error } => {
			format!("Volume {} error: {}", fingerprint.0, error)
		}

		// Job events
		Event::JobQueued { job_id, job_type } => {
			format!("Job queued: {} ({})", job_type, &job_id[..8])
		}
		Event::JobStarted { job_id, job_type } => {
			format!("Job started: {} ({})", job_type, &job_id[..8])
		}
		Event::JobProgress {
			job_id,
			job_type,
			progress,
			message,
			..
		} => {
			let msg = message
				.as_ref()
				.map(|m| format!(" - {}", m))
				.unwrap_or_default();
			format!(
				"Job progress: {} ({}) - {:.1}%{}",
				job_type,
				&job_id[..8],
				progress,
				msg
			)
		}
		Event::JobCompleted {
			job_id,
			job_type,
			output,
		} => {
			format!(
				"Job completed: {} ({}) - {:?}",
				job_type,
				&job_id[..8],
				output
			)
		}
		Event::JobFailed {
			job_id,
			job_type,
			error,
		} => {
			format!("Job failed: {} ({}) - {}", job_type, &job_id[..8], error)
		}
		Event::JobCancelled { job_id, job_type } => {
			format!("Job cancelled: {} ({})", job_type, &job_id[..8])
		}
		Event::JobPaused { job_id } => {
			format!("Job paused: {}", &job_id[..8])
		}
		Event::JobResumed { job_id } => {
			format!("Job resumed: {}", &job_id[..8])
		}

		// Indexing events
		Event::IndexingStarted { location_id } => {
			format!("Indexing started for location {}", location_id)
		}
		Event::IndexingProgress {
			location_id,
			processed,
			total,
		} => {
			let total_str = total.map(|t| format!("/{}", t)).unwrap_or_default();
			format!(
				"Indexing location {}: {}{} processed",
				location_id, processed, total_str
			)
		}
		Event::IndexingCompleted {
			location_id,
			total_files,
			total_dirs,
		} => {
			format!(
				"Indexing completed for location {}: {} files, {} dirs",
				location_id, total_files, total_dirs
			)
		}
		Event::IndexingFailed { location_id, error } => {
			format!("Indexing failed for location {}: {}", location_id, error)
		}

		// Device events
		Event::DeviceConnected {
			device_id,
			device_name,
		} => {
			format!("Device connected: {} ({})", device_name, device_id)
		}
		Event::DeviceDisconnected { device_id } => {
			format!("Device disconnected: {}", device_id)
		}

		// Resource events
		Event::ResourceChanged {
			resource_type,
			resource,
			..
		} => {
			if let Some(id) = resource.get("id") {
				format!("Resource changed: {} ({})", resource_type, id)
			} else {
				format!("Resource changed: {}", resource_type)
			}
		}
		Event::ResourceChangedBatch {
			resource_type,
			resources,
			..
		} => {
			if let Some(arr) = resources.as_array() {
				format!("Resources changed: {} ({} items)", resource_type, arr.len())
			} else {
				format!("Resources changed: {} (batch)", resource_type)
			}
		}
		Event::ResourceDeleted {
			resource_type,
			resource_id,
		} => {
			format!("Resource deleted: {} ({})", resource_type, resource_id)
		}

		// Legacy location events
		Event::LocationAdded {
			location_id, path, ..
		} => {
			format!("Location added: {} at {}", location_id, path.display())
		}
		Event::LocationRemoved { location_id, .. } => {
			format!("Location removed: {}", location_id)
		}
		Event::FilesIndexed {
			location_id, count, ..
		} => {
			format!("Files indexed: {} files at location {}", count, location_id)
		}
		Event::ThumbnailsGenerated { count, .. } => {
			format!("Thumbnails generated: {}", count)
		}
		Event::FileOperationCompleted {
			operation,
			affected_files,
			..
		} => {
			format!(
				"File operation completed: {:?} ({} files)",
				operation, affected_files
			)
		}
		Event::FilesModified { paths, .. } => {
			format!("Files modified: {} files", paths.len())
		}

		// Custom events
		Event::Custom { event_type, data } => {
			format!("Custom event: {} - {:?}", event_type, data)
		}
	}
}
