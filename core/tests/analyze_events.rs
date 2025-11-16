//! Event Log Analyzer for Sync Tests
//!
//! Parse events.log and sync_events.log (JSON lines) and generate condensed timeline

use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Debug)]
pub struct EventTimeline {
	pub resource_changed: HashMap<String, Vec<EventEntry>>,
	pub resource_deleted: HashMap<String, Vec<EventEntry>>,
	pub custom_events: Vec<EventEntry>,
}

#[derive(Debug, Clone)]
pub struct EventEntry {
	pub timestamp: String,
	pub event_type: String,
	pub resource_type: Option<String>,
	pub data: Value,
}

#[derive(Debug)]
pub struct SyncEventTimeline {
	pub state_changes: HashMap<String, Vec<SyncEventEntry>>,
	pub shared_changes: Vec<SyncEventEntry>,
	pub metrics_updates: Vec<SyncEventEntry>,
}

#[derive(Debug, Clone)]
pub struct SyncEventEntry {
	pub timestamp: String,
	pub event_type: String,
	pub library_id: Option<String>,
	pub model_type: Option<String>,
	pub device_id: Option<String>,
	pub data: Value,
}

impl EventTimeline {
	pub fn from_file(path: &Path) -> anyhow::Result<Self> {
		let file = std::fs::File::open(path)?;
		let reader = BufReader::new(file);

		let mut resource_changed: HashMap<String, Vec<EventEntry>> = HashMap::new();
		let mut resource_deleted: HashMap<String, Vec<EventEntry>> = HashMap::new();
		let mut custom_events = Vec::new();

		for (line_num, line) in reader.lines().enumerate() {
			let line = line?;
			let value: Value = match serde_json::from_str(&line) {
				Ok(v) => v,
				Err(e) => {
					eprintln!("Failed to parse line {}: {}", line_num + 1, e);
					continue;
				}
			};

			// Extract timestamp (approximate from system time if not in event)
			let timestamp = chrono::Utc::now().to_rfc3339();

			// Classify event
			if let Some(event_obj) = value.as_object() {
				if event_obj.contains_key("ResourceChanged") {
					if let Some(data) = event_obj.get("ResourceChanged") {
						if let Some(resource_type) =
							data.get("resource_type").and_then(|v| v.as_str())
						{
							resource_changed
								.entry(resource_type.to_string())
								.or_insert_with(Vec::new)
								.push(EventEntry {
									timestamp: timestamp.clone(),
									event_type: "ResourceChanged".to_string(),
									resource_type: Some(resource_type.to_string()),
									data: data.clone(),
								});
						}
					}
				} else if event_obj.contains_key("ResourceChangedBatch") {
					if let Some(data) = event_obj.get("ResourceChangedBatch") {
						if let Some(resource_type) =
							data.get("resource_type").and_then(|v| v.as_str())
						{
							resource_changed
								.entry(resource_type.to_string())
								.or_insert_with(Vec::new)
								.push(EventEntry {
									timestamp: timestamp.clone(),
									event_type: "ResourceChangedBatch".to_string(),
									resource_type: Some(resource_type.to_string()),
									data: data.clone(),
								});
						}
					}
				} else if event_obj.contains_key("ResourceDeleted") {
					if let Some(data) = event_obj.get("ResourceDeleted") {
						if let Some(resource_type) =
							data.get("resource_type").and_then(|v| v.as_str())
						{
							resource_deleted
								.entry(resource_type.to_string())
								.or_insert_with(Vec::new)
								.push(EventEntry {
									timestamp: timestamp.clone(),
									event_type: "ResourceDeleted".to_string(),
									resource_type: Some(resource_type.to_string()),
									data: data.clone(),
								});
						}
					}
				} else if event_obj.contains_key("Custom") {
					custom_events.push(EventEntry {
						timestamp,
						event_type: "Custom".to_string(),
						resource_type: None,
						data: value.clone(),
					});
				}
			}
		}

		Ok(Self {
			resource_changed,
			resource_deleted,
			custom_events,
		})
	}

	pub fn print_summary(&self) {
		println!("\n=== Event Timeline Summary ===\n");

		println!("Resource Changes:");
		for (resource_type, events) in &self.resource_changed {
			let batches = events
				.iter()
				.filter(|e| e.event_type == "ResourceChangedBatch")
				.count();
			let singles = events
				.iter()
				.filter(|e| e.event_type == "ResourceChanged")
				.count();
			println!(
				"  {}: {} events ({} batches, {} individual)",
				resource_type,
				events.len(),
				batches,
				singles
			);
		}

		println!("\nResource Deletions:");
		for (resource_type, events) in &self.resource_deleted {
			println!("  {}: {} deletions", resource_type, events.len());
		}

		println!("\nCustom Events: {}", self.custom_events.len());

		println!("\n=== Detailed Timeline ===\n");

		// Print chronological timeline (approximate since we don't have real timestamps)
		for (resource_type, events) in &self.resource_changed {
			if events.len() > 10 {
				println!(
					"{}: {} events (showing first 5 and last 5)",
					resource_type,
					events.len()
				);
				for event in events.iter().take(5) {
					self.print_event(event);
				}
				println!("  ... {} more events ...", events.len() - 10);
				for event in events.iter().rev().take(5).rev() {
					self.print_event(event);
				}
			} else {
				println!("{}: {} events", resource_type, events.len());
				for event in events {
					self.print_event(event);
				}
			}
			println!();
		}
	}

	fn print_event(&self, event: &EventEntry) {
		match event.event_type.as_str() {
			"ResourceChangedBatch" => {
				let count = event
					.data
					.get("resources")
					.and_then(|v| v.as_array())
					.map(|a| a.len())
					.unwrap_or(0);
				println!("  [BATCH] {} items", count);
			}
			"ResourceChanged" => {
				let resource_id = event
					.data
					.get("resource_id")
					.and_then(|v| v.as_str())
					.unwrap_or("unknown");
				println!("  [SINGLE] {}", resource_id);
			}
			_ => {
				println!("  [{}] {:?}", event.event_type, event.data);
			}
		}
	}

	pub fn count_by_type(&self, resource_type: &str) -> usize {
		self.resource_changed
			.get(resource_type)
			.map(|v| v.len())
			.unwrap_or(0)
	}
}

impl SyncEventTimeline {
	pub fn from_file(path: &Path) -> anyhow::Result<Self> {
		let file = std::fs::File::open(path)?;
		let reader = BufReader::new(file);

		let mut state_changes: HashMap<String, Vec<SyncEventEntry>> = HashMap::new();
		let mut shared_changes = Vec::new();
		let mut metrics_updates = Vec::new();

		for (line_num, line) in reader.lines().enumerate() {
			let line = line?;
			let value: Value = match serde_json::from_str(&line) {
				Ok(v) => v,
				Err(e) => {
					eprintln!("Failed to parse sync event line {}: {}", line_num + 1, e);
					continue;
				}
			};

			// Parse sync event types
			if let Some(event_type) = value.get("type").and_then(|v| v.as_str()) {
				match event_type {
					"state_change" => {
						let library_id = value
							.get("library_id")
							.and_then(|v| v.as_str())
							.map(String::from);
						let model_type = value
							.get("model_type")
							.and_then(|v| v.as_str())
							.map(String::from);
						let device_id = value
							.get("device_id")
							.and_then(|v| v.as_str())
							.map(String::from);
						let timestamp = value
							.get("timestamp")
							.and_then(|v| v.as_str())
							.unwrap_or("unknown")
							.to_string();

						let entry = SyncEventEntry {
							timestamp,
							event_type: "StateChange".to_string(),
							library_id,
							model_type: model_type.clone(),
							device_id,
							data: value.clone(),
						};

						if let Some(mt) = model_type {
							state_changes.entry(mt).or_insert_with(Vec::new).push(entry);
						}
					}
					"shared_change" => {
						let library_id = value
							.get("library_id")
							.and_then(|v| v.as_str())
							.map(String::from);
						let timestamp = chrono::Utc::now().to_rfc3339();

						shared_changes.push(SyncEventEntry {
							timestamp,
							event_type: "SharedChange".to_string(),
							library_id,
							model_type: None,
							device_id: None,
							data: value.clone(),
						});
					}
					"metrics_updated" => {
						let library_id = value
							.get("library_id")
							.and_then(|v| v.as_str())
							.map(String::from);
						let timestamp = chrono::Utc::now().to_rfc3339();

						metrics_updates.push(SyncEventEntry {
							timestamp,
							event_type: "MetricsUpdated".to_string(),
							library_id,
							model_type: None,
							device_id: None,
							data: value.clone(),
						});
					}
					_ => {
						eprintln!("Unknown sync event type: {}", event_type);
					}
				}
			}
		}

		Ok(Self {
			state_changes,
			shared_changes,
			metrics_updates,
		})
	}

	pub fn print_summary(&self) {
		println!("\n=== Sync Event Timeline Summary ===\n");

		println!("State Changes (device-owned):");
		for (model_type, events) in &self.state_changes {
			println!("  {}: {} state changes", model_type, events.len());
		}

		println!(
			"\nShared Changes (HLC-ordered): {}",
			self.shared_changes.len()
		);
		println!("Metrics Updates: {}", self.metrics_updates.len());

		println!("\n=== Detailed Sync Timeline ===\n");

		// Print state changes by model type
		for (model_type, events) in &self.state_changes {
			if events.len() > 10 {
				println!(
					"{}: {} state changes (showing first 5 and last 5)",
					model_type,
					events.len()
				);
				for event in events.iter().take(5) {
					self.print_sync_event(event);
				}
				println!("  ... {} more state changes ...", events.len() - 10);
				for event in events.iter().rev().take(5).rev() {
					self.print_sync_event(event);
				}
			} else {
				println!("{}: {} state changes", model_type, events.len());
				for event in events {
					self.print_sync_event(event);
				}
			}
			println!();
		}

		// Print shared changes (if any)
		if !self.shared_changes.is_empty() {
			if self.shared_changes.len() > 10 {
				println!(
					"Shared Changes: {} (showing first 5 and last 5)",
					self.shared_changes.len()
				);
				for event in self.shared_changes.iter().take(5) {
					self.print_sync_event(event);
				}
				println!(
					"  ... {} more shared changes ...",
					self.shared_changes.len() - 10
				);
				for event in self.shared_changes.iter().rev().take(5).rev() {
					self.print_sync_event(event);
				}
			} else {
				println!("Shared Changes: {}", self.shared_changes.len());
				for event in &self.shared_changes {
					self.print_sync_event(event);
				}
			}
			println!();
		}
	}

	fn print_sync_event(&self, event: &SyncEventEntry) {
		match event.event_type.as_str() {
			"StateChange" => {
				let record_uuid = event
					.data
					.get("record_uuid")
					.and_then(|v| v.as_str())
					.unwrap_or("unknown");
				let device_id = event.device_id.as_deref().unwrap_or("unknown");
				println!(
					"  [STATE] {} -> {} (device: {})",
					event.model_type.as_deref().unwrap_or("unknown"),
					&record_uuid[..8],
					&device_id[..8]
				);
			}
			"SharedChange" => {
				let entry = event.data.get("entry");
				let model_type = entry
					.and_then(|e| e.get("model_type"))
					.and_then(|v| v.as_str())
					.unwrap_or("unknown");
				let record_uuid = entry
					.and_then(|e| e.get("record_uuid"))
					.and_then(|v| v.as_str())
					.unwrap_or("unknown");
				println!("  [SHARED] {} -> {}", model_type, &record_uuid[..8]);
			}
			"MetricsUpdated" => {
				println!("  [METRICS] Sync metrics snapshot");
			}
			_ => {
				println!("  [{}] {:?}", event.event_type, event.data);
			}
		}
	}

	pub fn count_state_changes_by_type(&self, model_type: &str) -> usize {
		self.state_changes
			.get(model_type)
			.map(|v| v.len())
			.unwrap_or(0)
	}

	pub fn total_state_changes(&self) -> usize {
		self.state_changes.values().map(|v| v.len()).sum()
	}
}

// Usage example in tests:
#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn example_usage() {
		let events_path = std::path::Path::new("events.log");
		let sync_events_path = std::path::Path::new("sync_events.log");

		// Parse main event bus events
		if events_path.exists() {
			println!("\n=== MAIN EVENT BUS ===");
			let timeline = EventTimeline::from_file(events_path).unwrap();
			timeline.print_summary();

			// Query specific metrics
			let entry_events = timeline.count_by_type("entry");
			let content_id_events = timeline.count_by_type("content_identity");

			println!("\nMetrics:");
			println!("  Entry events: {}", entry_events);
			println!("  Content_identity events: {}", content_id_events);
		} else {
			println!("events.log not found");
		}

		// Parse sync event bus events
		if sync_events_path.exists() {
			println!("\n=== SYNC EVENT BUS ===");
			let sync_timeline = SyncEventTimeline::from_file(sync_events_path).unwrap();
			sync_timeline.print_summary();

			// Query specific metrics
			let entry_state_changes = sync_timeline.count_state_changes_by_type("entry");
			let location_state_changes = sync_timeline.count_state_changes_by_type("location");
			let total_state_changes = sync_timeline.total_state_changes();

			println!("\nSync Metrics:");
			println!("  Entry state changes: {}", entry_state_changes);
			println!("  Location state changes: {}", location_state_changes);
			println!("  Total state changes: {}", total_state_changes);
			println!("  Shared changes: {}", sync_timeline.shared_changes.len());
		} else {
			println!("sync_events.log not found");
		}
	}

	#[test]
	fn analyze_snapshot() {
		// Analyze a specific snapshot directory
		let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
		let snapshot_base_path = format!(
			"{}/Library/Application Support/spacedrive/sync_tests/snapshots",
			home
		);
		let snapshot_base = std::path::Path::new(&snapshot_base_path);

		if !snapshot_base.exists() {
			println!("No snapshots found at {:?}", snapshot_base);
			return;
		}

		// Find the most recent snapshot
		let mut entries: Vec<_> = std::fs::read_dir(snapshot_base)
			.unwrap()
			.filter_map(|e| e.ok())
			.filter(|e| e.path().is_dir())
			.collect();

		entries.sort_by_key(|e| std::cmp::Reverse(e.path()));

		if entries.is_empty() {
			println!("No snapshot directories found");
			return;
		}

		let latest_snapshot = entries[0].path();
		println!("\n=== ANALYZING LATEST SNAPSHOT ===");
		println!("Snapshot: {:?}\n", latest_snapshot);

		// Check for nested final_state directory (newer snapshots)
		let base_path = if latest_snapshot.join("final_state").exists() {
			latest_snapshot.join("final_state")
		} else {
			latest_snapshot.clone()
		};

		// Analyze Alice's events
		println!("\n--- ALICE ---");
		let alice_events = base_path.join("alice/events.log");
		let alice_sync_events = base_path.join("alice/sync_events.log");

		if alice_events.exists() {
			let timeline = EventTimeline::from_file(&alice_events).unwrap();
			println!(
				"Main events: {} resource changes, {} deletions, {} custom",
				timeline
					.resource_changed
					.values()
					.map(|v| v.len())
					.sum::<usize>(),
				timeline
					.resource_deleted
					.values()
					.map(|v| v.len())
					.sum::<usize>(),
				timeline.custom_events.len()
			);
		}

		if alice_sync_events.exists() {
			let sync_timeline = SyncEventTimeline::from_file(&alice_sync_events).unwrap();
			println!(
				"Sync events: {} state changes, {} shared changes, {} metrics updates",
				sync_timeline.total_state_changes(),
				sync_timeline.shared_changes.len(),
				sync_timeline.metrics_updates.len()
			);
		}

		// Analyze Bob's events
		println!("\n--- BOB ---");
		let bob_events = base_path.join("bob/events.log");
		let bob_sync_events = base_path.join("bob/sync_events.log");

		if bob_events.exists() {
			let timeline = EventTimeline::from_file(&bob_events).unwrap();
			println!(
				"Main events: {} resource changes, {} deletions, {} custom",
				timeline
					.resource_changed
					.values()
					.map(|v| v.len())
					.sum::<usize>(),
				timeline
					.resource_deleted
					.values()
					.map(|v| v.len())
					.sum::<usize>(),
				timeline.custom_events.len()
			);
		}

		if bob_sync_events.exists() {
			let sync_timeline = SyncEventTimeline::from_file(&bob_sync_events).unwrap();
			println!(
				"Sync events: {} state changes, {} shared changes, {} metrics updates",
				sync_timeline.total_state_changes(),
				sync_timeline.shared_changes.len(),
				sync_timeline.metrics_updates.len()
			);
		}
	}
}
