mod args;

use anyhow::Result;
use clap::Subcommand;
use chrono::{DateTime, Utc};
use serde_json;
use std::time::Duration;
use tokio::time::sleep;

use crate::util::prelude::*;
use crate::context::Context;
use sd_core::ops::sync::get_metrics::{GetSyncMetrics, GetSyncMetricsInput, GetSyncMetricsOutput};

use self::args::*;

#[derive(Subcommand, Debug)]
pub enum SyncCmd {
	/// Show sync metrics
	Metrics(SyncMetricsArgs),
}

pub async fn run(ctx: &Context, cmd: SyncCmd) -> Result<()> {
	match cmd {
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
	let library_id = ctx.library_id
		.ok_or_else(|| anyhow::anyhow!("No library selected. Use 'sd library switch' to select a library first."))?;

	// Call the sync metrics API
	let input = GetSyncMetricsInput {
		since,
		peer_id,
		model_type: args.model.clone(),
		state_only: if args.state { Some(true) } else { None },
		operations_only: if args.operations { Some(true) } else { None },
		errors_only: if args.errors { Some(true) } else { None },
	};

	let query = GetSyncMetrics::from_input(input)
		.map_err(|e| anyhow::anyhow!("Failed to create query: {}", e))?;

	let output: GetSyncMetricsOutput = ctx
		.core
		.query(&query, Some(library_id))
		.await?;

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
	let library_id = ctx.library_id
		.ok_or_else(|| anyhow::anyhow!("No library selected. Use 'sd library switch' to select a library first."))?;

	println!("Watching sync metrics for library {} (Press Ctrl+C to stop)...", library_id);
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

	let value: i64 = parts[0].parse()
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

fn display_metrics(snapshot: &sd_core::service::sync::metrics::snapshot::SyncMetricsSnapshot, args: &SyncMetricsArgs) {
	// Display state metrics
	if !args.operations && !args.errors {
		println!("📊 State Metrics");
		println!("===============");
		println!("Current State: {:?}", snapshot.state.current_state);
		println!("Total Transitions: {}", snapshot.state.total_transitions);
		println!();
		
		if !snapshot.state.transition_counts.is_empty() {
			println!("State Transitions:");
			for ((from, to), count) in &snapshot.state.transition_counts {
				println!("  {:?} → {:?}: {}", from, to, count);
			}
			println!();
		}

		if !snapshot.state.time_in_states.is_empty() {
			println!("Time in States:");
			for (state, duration) in &snapshot.state.time_in_states {
				println!("  {:?}: {:.2}s", state, duration.as_secs_f64());
			}
			println!();
		}
	}

	// Display operation metrics
	if !args.state && !args.errors {
		println!("⚡ Operation Metrics");
		println!("===================");
		println!("Broadcasts Sent: {}", snapshot.operations.broadcasts_sent);
		println!("Broadcasts Failed: {}", snapshot.operations.broadcasts_failed);
		println!("Changes Received: {}", snapshot.operations.changes_received);
		println!("Changes Applied: {}", snapshot.operations.changes_applied);
		println!("Backfill Sessions: {}", snapshot.operations.backfill_sessions);
		println!("Backfill Rounds: {}", snapshot.operations.backfill_rounds);
		println!();

		if !snapshot.operations.entries_synced_by_model.is_empty() {
			println!("Entries Synced by Model:");
			for (model, count) in &snapshot.operations.entries_synced_by_model {
				println!("  {}: {}", model, count);
			}
			println!();
		}
	}

	// Display data volume metrics
	if !args.state && !args.operations && !args.errors {
		println!("📈 Data Volume Metrics");
		println!("=====================");
		println!("Total Data Synced: {} bytes", snapshot.data_volume.total_bytes_synced);
		println!("State Changes: {} bytes", snapshot.data_volume.state_changes_bytes);
		println!("Shared Resources: {} bytes", snapshot.data_volume.shared_resources_bytes);
		println!();
	}

	// Display performance metrics
	if !args.state && !args.operations && !args.errors {
		println!("🚀 Performance Metrics");
		println!("=====================");
		println!("Average Broadcast Latency: {:.2}ms", snapshot.performance.avg_broadcast_latency_ms);
		println!("Average Apply Latency: {:.2}ms", snapshot.performance.avg_apply_latency_ms);
		println!("Max Watermark: {}", snapshot.performance.max_watermark);
		println!();
	}

	// Display error metrics
	if !args.state && !args.operations {
		println!("❌ Error Metrics");
		println!("================");
		println!("Total Errors: {}", snapshot.errors.total_errors);
		println!("Broadcast Errors: {}", snapshot.errors.broadcast_errors);
		println!("Apply Errors: {}", snapshot.errors.apply_errors);
		println!("Backfill Errors: {}", snapshot.errors.backfill_errors);
		println!();

		if !snapshot.errors.recent_errors.is_empty() {
			println!("Recent Errors:");
			for error in &snapshot.errors.recent_errors {
				println!("  [{}] {}: {}", 
					error.timestamp.format("%H:%M:%S"),
					error.error_type,
					error.message
				);
				if let Some(model) = &error.model_type {
					println!("    Model: {}", model);
				}
			}
			println!();
		}
	}
}