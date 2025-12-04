mod args;

use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Subcommand;
use comfy_table::{presets::UTF8_FULL, Attribute, Cell, Color, ContentArrangement, Row, Table};
use crossterm::style::Stylize;
use serde_json;
use std::time::Duration;
use tokio::time::sleep;

use crate::context::Context;
use crate::util::prelude::*;
use sd_core::infra::sync::{EventSeverity, SyncEventQuery, SyncEventType};
use sd_core::ops::sync::get_metrics::GetSyncMetricsInput;
use sd_core::service::sync::state::DeviceSyncState;

use self::args::*;

#[derive(Subcommand, Debug)]
pub enum SyncCmd {
	/// Show sync metrics
	Metrics(SyncMetricsArgs),

	/// Export sync event log
	Events(SyncEventsArgs),
}

pub async fn run(ctx: &Context, cmd: SyncCmd) -> Result<()> {
	match cmd {
		SyncCmd::Events(args) => export_events(ctx, args).await?,
		SyncCmd::Metrics(args) => {
			// Parse time filters
			let since = if let Some(since_str) = &args.since {
				Some(parse_time_filter(since_str)?)
			} else {
				None
			};

			// Parse peer ID filter
			let peer_id = if let Some(peer_str) = &args.peer {
				Some(uuid::Uuid::parse_str(peer_str)?)
			} else {
				None
			};

			if args.watch {
				run_watch_mode(ctx, since, peer_id, &args).await?;
			} else {
				run_single_query(ctx, since, peer_id, &args).await?;
			}
		}
	}
	Ok(())
}

async fn run_single_query(
	ctx: &Context,
	since: Option<DateTime<Utc>>,
	peer_id: Option<uuid::Uuid>,
	args: &SyncMetricsArgs,
) -> Result<()> {
	// Check if we have a library ID
	let library_id = ctx.library_id.ok_or_else(|| {
		anyhow::anyhow!("No library selected. Use 'sd library switch' to select a library first.")
	})?;

	// Call the sync metrics API
	let input = GetSyncMetricsInput {
		since,
		peer_id,
		model_type: args.model.clone(),
		state_only: if args.state { Some(true) } else { None },
		operations_only: if args.operations { Some(true) } else { None },
		errors_only: if args.errors { Some(true) } else { None },
	};

	let json_response = ctx.core.query(&input, Some(library_id)).await?;
	let output: sd_core::ops::sync::get_metrics::GetSyncMetricsOutput =
		serde_json::from_value(json_response)?;

	if args.json {
		println!("{}", serde_json::to_string_pretty(&output.metrics)?);
	} else {
		display_metrics(&output.metrics, args);
	}

	Ok(())
}

async fn run_watch_mode(
	ctx: &Context,
	since: Option<DateTime<Utc>>,
	peer_id: Option<uuid::Uuid>,
	args: &SyncMetricsArgs,
) -> Result<()> {
	// Check if we have a library ID
	let library_id = ctx.library_id.ok_or_else(|| {
		anyhow::anyhow!("No library selected. Use 'sd library switch' to select a library first.")
	})?;

	println!(
		"Watching sync metrics for library {} (Press Ctrl+C to stop)...",
		library_id
	);
	println!();

	loop {
		// Clear screen for watch mode
		print!("\x1B[2J\x1B[1;1H");
		println!("Sync Metrics - Live View");
		println!("========================");
		println!("Library: {}", library_id);
		println!("Updated: {}", Utc::now().format("%Y-%m-%d %H:%M:%S UTC"));
		println!();

		// Get current metrics
		if let Err(e) = run_single_query(ctx, since, peer_id, args).await {
			eprintln!("Error fetching metrics: {}", e);
		}

		println!();
		println!("Refreshing in 2 seconds...");

		// Wait before next update
		sleep(Duration::from_secs(2)).await;
	}
}

fn parse_time_filter(time_str: &str) -> Result<DateTime<Utc>> {
	// Try parsing as relative time first
	if time_str.ends_with(" ago") {
		let duration_str = &time_str[..time_str.len() - 4];
		let duration = parse_duration(duration_str)?;
		Ok(Utc::now() - duration)
	} else {
		// Try parsing as absolute time
		DateTime::parse_from_rfc3339(time_str)
			.map(|dt| dt.with_timezone(&Utc))
			.or_else(|_| {
				// Try common formats
				DateTime::parse_from_str(time_str, "%Y-%m-%d %H:%M:%S")
					.map(|dt| dt.with_timezone(&Utc))
			})
			.map_err(|_| anyhow::anyhow!("Invalid time format: {}", time_str))
	}
}

fn parse_duration(duration_str: &str) -> Result<chrono::Duration> {
	let parts: Vec<&str> = duration_str.split_whitespace().collect();
	if parts.len() != 2 {
		return Err(anyhow::anyhow!("Invalid duration format: {}", duration_str));
	}

	let value: i64 = parts[0]
		.parse()
		.map_err(|_| anyhow::anyhow!("Invalid number: {}", parts[0]))?;
	let unit = parts[1].to_lowercase();

	let seconds = match unit.as_str() {
		"second" | "seconds" | "sec" | "s" => value,
		"minute" | "minutes" | "min" | "m" => value * 60,
		"hour" | "hours" | "h" => value * 3600,
		"day" | "days" | "d" => value * 86400,
		"week" | "weeks" | "w" => value * 604800,
		_ => return Err(anyhow::anyhow!("Unknown time unit: {}", unit)),
	};

	Ok(chrono::Duration::seconds(seconds))
}

fn display_metrics(
	snapshot: &sd_core::service::sync::metrics::snapshot::SyncMetricsSnapshot,
	args: &SyncMetricsArgs,
) {
	// Status header box
	let status_icon = match snapshot.state.current_state {
		DeviceSyncState::Ready => "●".green(),
		DeviceSyncState::Backfilling { .. } => "◐".yellow(),
		DeviceSyncState::CatchingUp { .. } => "◔".yellow(),
		DeviceSyncState::Uninitialized => "○".dark_grey(),
		DeviceSyncState::Paused => "◦".dark_grey(),
	};

	let state_name = format_state(&snapshot.state.current_state);
	let backfill_status = if snapshot.operations.backfill_sessions_completed > 0 {
		format!(
			"✓ Complete ({} rounds, {} entries)",
			snapshot.operations.backfill_pagination_rounds,
			snapshot.data_volume.entries_synced.values().sum::<u64>()
		)
		.green()
	} else if snapshot.operations.active_backfill_sessions > 0 {
		format!(
			"◐ In Progress ({} active)",
			snapshot.operations.active_backfill_sessions
		)
		.yellow()
	} else {
		"Not started".to_string().dark_grey()
	};

	let mut status_table = Table::new();
	status_table
		.load_preset(UTF8_FULL)
		.set_content_arrangement(ContentArrangement::Dynamic)
		.set_header(Row::from(vec![
			Cell::new("SYNC STATUS").add_attribute(Attribute::Bold)
		]));

	status_table.add_row(vec![format!(
		"{} {}                                    Uptime: {}",
		status_icon,
		state_name,
		format_duration(snapshot.state.uptime_seconds)
	)]);

	if !snapshot.state.state_history.is_empty() {
		let last_transition = snapshot.state.state_history.last();
		if let Some(transition) = last_transition {
			status_table.add_row(vec![format!(
				"Last transition: {} → {}",
				format_state(&transition.from),
				format_state(&transition.to)
			)]);
		}
	}

	status_table.add_row(vec![format!("Backfill: {}", backfill_status)]);

	println!("{}", status_table);
	println!();

	// Metrics grid
	if !args.state && !args.operations && !args.errors {
		let mut grid_table = Table::new();
		grid_table
			.load_preset(UTF8_FULL)
			.set_content_arrangement(ContentArrangement::Dynamic);

		grid_table.set_header(Row::from(vec![
			Cell::new("ACTIVITY").add_attribute(Attribute::Bold),
			Cell::new("DATA VOLUME").add_attribute(Attribute::Bold),
			Cell::new("PERFORMANCE").add_attribute(Attribute::Bold),
		]));

		grid_table.add_row(vec![
			format!(
				"Broadcasts sent      {:>6}",
				snapshot.operations.broadcasts_sent
			),
			format!(
				"Sent         {:>10}",
				format_bytes(snapshot.data_volume.bytes_sent)
			),
			format!(
				"Broadcast   {:>6.1}ms",
				snapshot.performance.broadcast_latency.avg_ms
			),
		]);

		grid_table.add_row(vec![
			format!(
				"Changes received     {:>6}",
				snapshot.operations.changes_received
			),
			format!(
				"Received     {:>10}",
				format_bytes(snapshot.data_volume.bytes_received)
			),
			format!(
				"Apply       {:>6.1}ms",
				snapshot.performance.apply_latency.avg_ms
			),
		]);

		grid_table.add_row(vec![
			format!(
				"Changes applied      {:>6}",
				snapshot.operations.changes_applied
			),
			"".to_string(),
			"".to_string(),
		]);

		grid_table.add_row(vec![
			format!(
				"Backfill sessions    {:>6}",
				snapshot.operations.backfill_sessions_completed
			),
			"".to_string(),
			"".to_string(),
		]);

		println!("{}", grid_table);
		println!();
	}

	// Synced entries
	if !args.state && !args.errors && !snapshot.data_volume.entries_synced.is_empty() {
		let mut entries_table = Table::new();
		entries_table
			.load_preset(UTF8_FULL)
			.set_content_arrangement(ContentArrangement::Dynamic)
			.set_header(Row::from(vec![
				Cell::new("SYNCED ENTRIES").add_attribute(Attribute::Bold)
			]));

		let mut entries: Vec<_> = snapshot.data_volume.entries_synced.iter().collect();
		entries.sort_by(|a, b| b.1.cmp(a.1));

		let entries_line = entries
			.iter()
			.map(|(model, count)| format!("{}: {}", model, count))
			.collect::<Vec<_>>()
			.join("  │  ");

		entries_table.add_row(vec![entries_line]);

		println!("{}", entries_table);
		println!();
	}

	// Errors
	if !args.state && !args.operations {
		let has_errors = snapshot.errors.total_errors > 0;
		let error_summary = if has_errors {
			format!(
				"Network: {} · Database: {} · Apply: {} · Validation: {}",
				snapshot.errors.network_errors,
				snapshot.errors.database_errors,
				snapshot.errors.apply_errors,
				snapshot.errors.validation_errors
			)
			.red()
		} else {
			"No errors".to_string().green()
		};

		println!("{}", error_summary);

		if !snapshot.errors.recent_errors.is_empty() {
			println!();
			let mut error_table = Table::new();
			error_table
				.load_preset(UTF8_FULL)
				.set_content_arrangement(ContentArrangement::Dynamic)
				.set_header(Row::from(vec![
					Cell::new("Time").add_attribute(Attribute::Bold),
					Cell::new("Type").add_attribute(Attribute::Bold),
					Cell::new("Message").add_attribute(Attribute::Bold),
				]));

			for error in snapshot.errors.recent_errors.iter().take(5) {
				error_table.add_row(vec![
					error.timestamp.format("%H:%M:%S").to_string(),
					error.error_type.clone(),
					error.message.clone(),
				]);
			}

			println!("{}", error_table);
		}
	}
}

// Helper functions
fn format_state(state: &DeviceSyncState) -> String {
	match state {
		DeviceSyncState::Ready => "Ready".to_string(),
		DeviceSyncState::Uninitialized => "Uninitialized".to_string(),
		DeviceSyncState::Paused => "Paused".to_string(),
		DeviceSyncState::Backfilling { peer, progress } => {
			format!("Backfilling ({}%)", progress)
		}
		DeviceSyncState::CatchingUp { buffered_count } => {
			if *buffered_count > 0 {
				format!("CatchingUp ({} buffered)", buffered_count)
			} else {
				"CatchingUp".to_string()
			}
		}
	}
}

fn format_duration(seconds: u64) -> String {
	if seconds < 60 {
		format!("{}s", seconds)
	} else if seconds < 3600 {
		format!("{}m {}s", seconds / 60, seconds % 60)
	} else {
		format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
	}
}

fn format_bytes(bytes: u64) -> String {
	const KB: u64 = 1024;
	const MB: u64 = KB * 1024;
	const GB: u64 = MB * 1024;

	if bytes == 0 {
		"0 B".to_string()
	} else if bytes < KB {
		format!("{} B", bytes)
	} else if bytes < MB {
		format!("{:.1} KB", bytes as f64 / KB as f64)
	} else if bytes < GB {
		format!("{:.1} MB", bytes as f64 / MB as f64)
	} else {
		format!("{:.2} GB", bytes as f64 / GB as f64)
	}
}

async fn export_events(ctx: &Context, args: SyncEventsArgs) -> Result<()> {
	let library_id = ctx.library_id.ok_or_else(|| {
		anyhow::anyhow!("No library selected. Use 'sd library switch' to select a library first.")
	})?;

	// Build query
	let mut query = SyncEventQuery::new(library_id).with_limit(args.limit);

	// Parse time filter
	if let Some(since_str) = &args.since {
		let since = parse_time_filter(since_str)?;
		query = query.with_time_range(since, Utc::now());
	}

	// Parse event type filter
	if let Some(event_type_str) = &args.event_type {
		if let Some(event_type) = SyncEventType::from_str(event_type_str) {
			query = query.with_event_types(vec![event_type]);
		} else {
			return Err(anyhow::anyhow!("Invalid event type: {}", event_type_str));
		}
	}

	// Parse correlation ID filter
	if let Some(corr_id_str) = &args.correlation_id {
		let corr_id = uuid::Uuid::parse_str(corr_id_str)?;
		query = query.with_correlation_id(corr_id);
	}

	// Parse severity filter
	if let Some(severity_str) = &args.severity {
		if let Some(severity) = EventSeverity::from_str(severity_str) {
			query = query.with_severities(vec![severity]);
		} else {
			return Err(anyhow::anyhow!("Invalid severity: {}", severity_str));
		}
	}

	// Call the API
	let input = sd_core::ops::sync::get_event_log::GetSyncEventLogInput {
		start_time: query.time_range.map(|(start, _)| start),
		end_time: query.time_range.map(|(_, end)| end),
		event_types: query.event_types,
		categories: query.categories,
		severities: query.severities,
		peer_id: query.peer_filter,
		model_type: query.model_type_filter,
		correlation_id: query.correlation_id,
		limit: query.limit,
		offset: query.offset,
		include_remote_peers: if args.all_devices { Some(true) } else { None },
	};

	let json_response = ctx.core.query(&input, Some(library_id)).await?;
	let output: sd_core::ops::sync::get_event_log::GetSyncEventLogOutput =
		serde_json::from_value(json_response)?;

	// Format output
	let formatted = match args.format.as_str() {
		"json" => format_events_json(&output.events, args.with_device)?,
		"sql" => format_events_sql(&output.events),
		"markdown" | "md" => format_events_markdown(&output.events, args.with_device),
		_ => {
			return Err(anyhow::anyhow!(
				"Unknown format: {}. Use json, sql, or markdown",
				args.format
			))
		}
	};

	// Write output
	if let Some(output_file) = &args.output {
		std::fs::write(output_file, formatted)?;
		println!("Exported {} events to {}", output.events.len(), output_file);
	} else {
		println!("{}", formatted);
	}

	Ok(())
}

fn format_events_json(
	events: &[sd_core::infra::sync::SyncEventLog],
	with_device: bool,
) -> Result<String> {
	if with_device {
		Ok(serde_json::to_string_pretty(events)?)
	} else {
		// Strip device_id for cleaner output
		let simplified: Vec<_> = events
			.iter()
			.map(|e| {
				serde_json::json!({
					"timestamp": e.timestamp,
					"event_type": e.event_type,
					"severity": e.severity,
					"summary": e.summary,
					"details": e.details,
					"correlation_id": e.correlation_id,
					"peer_device_id": e.peer_device_id,
					"record_count": e.record_count,
					"duration_ms": e.duration_ms,
				})
			})
			.collect();
		Ok(serde_json::to_string_pretty(&simplified)?)
	}
}

fn format_events_sql(events: &[sd_core::infra::sync::SyncEventLog]) -> String {
	let mut output = String::from("-- Sync Event Log Export\n");
	output.push_str("-- Generated: ");
	output.push_str(&Utc::now().to_rfc3339());
	output.push_str("\n\n");

	for event in events {
		let details = event
			.details
			.as_ref()
			.map(|d| serde_json::to_string(d).unwrap_or_default())
			.unwrap_or_default();

		let model_types = event
			.model_types
			.as_ref()
			.map(|m| m.join(","))
			.unwrap_or_default();

		output.push_str(&format!(
			"INSERT INTO sync_event_log (timestamp, device_id, event_type, category, severity, summary, details, correlation_id, peer_device_id, model_types, record_count, duration_ms) VALUES ('{}', '{}', '{}', '{}', '{}', '{}', {}, {}, {}, {}, {}, {});\n",
			event.timestamp.to_rfc3339(),
			event.device_id,
			event.event_type.as_str(),
			event.category.as_str(),
			event.severity.as_str(),
			event.summary.replace("'", "''"), // Escape quotes
			if details.is_empty() { "NULL".to_string() } else { format!("'{}'", details.replace("'", "''")) },
			event.correlation_id.map(|id| format!("'{}'", id)).unwrap_or_else(|| "NULL".to_string()),
			event.peer_device_id.map(|id| format!("'{}'", id)).unwrap_or_else(|| "NULL".to_string()),
			if model_types.is_empty() { "NULL".to_string() } else { format!("'{}'", model_types) },
			event.record_count.map(|c| c.to_string()).unwrap_or_else(|| "NULL".to_string()),
			event.duration_ms.map(|d| d.to_string()).unwrap_or_else(|| "NULL".to_string()),
		));
	}

	output
}

fn format_events_markdown(
	events: &[sd_core::infra::sync::SyncEventLog],
	with_device: bool,
) -> String {
	let mut output = String::from("# Sync Event Log\n\n");
	output.push_str(&format!("**Exported**: {}\n", Utc::now().to_rfc3339()));
	output.push_str(&format!("**Event Count**: {}\n\n", events.len()));

	output.push_str("| Timestamp | Event Type | Severity | Summary |\n");
	output.push_str("|-----------|------------|----------|----------|\n");

	for event in events {
		output.push_str(&format!(
			"| {} | {:?} | {:?} | {} |\n",
			event.timestamp.format("%Y-%m-%d %H:%M:%S"),
			event.event_type,
			event.severity,
			event.summary
		));
	}

	if with_device && !events.is_empty() {
		output.push_str("\n## Devices\n\n");
		let device_ids: std::collections::HashSet<_> = events.iter().map(|e| e.device_id).collect();
		for device_id in device_ids {
			output.push_str(&format!("- {}\n", device_id));
		}
	}

	output
}
