use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod sources;

pub fn derive_hardware_label_from_paths(paths: &[PathBuf]) -> Option<String> {
	use sysinfo::{DiskKind, Disks};
	if paths.is_empty() {
		return None;
	}
	let p = &paths[0];
	let disks = Disks::new_with_refreshed_list();
	// Choose disk with longest mount point prefix match
	let mut best: Option<(usize, String, DiskKind, bool)> = None; // (prefix_len, name, kind, removable)
	for d in disks.list() {
		let mp = d.mount_point();
		if p.starts_with(mp) {
			let len = mp.as_os_str().len();
			let name = d.name().to_string_lossy().to_string();
			let kind = d.kind();
			let removable = d.is_removable();
			if best.as_ref().map(|(l, _, _, _)| *l).unwrap_or(0) < len {
				best = Some((len, name, kind, removable));
			}
		}
	}
	if let Some((_, name, kind, removable)) = best {
		let class = match kind {
			DiskKind::HDD => "HDD",
			DiskKind::SSD => "SSD",
			_ => "Disk",
		};
		let scope = if removable { "External" } else { "Internal" };
		return Some(format!("{} {} ({})", scope, class, name));
	}
	None
}

// Normalized v2 model for per-scenario results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMeta {
	pub id: uuid::Uuid,
	pub recipe_name: String,
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub location_paths: Vec<PathBuf>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub hardware_label: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub timestamp_utc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Durations {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub discovery_s: Option<f64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub processing_s: Option<f64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub content_s: Option<f64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub total_s: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "scenario", rename_all = "kebab-case")]
pub enum BenchmarkRun {
	IndexingDiscovery {
		meta: RunMeta,
		files: u64,
		files_per_s: f64,
		dirs: u64,
		dirs_per_s: f64,
		total_gb: f64,
		errors: u64,
		durations: Durations,
	},
	Aggregation {
		meta: RunMeta,
		files: u64,
		files_per_s: f64,
		dirs: u64,
		dirs_per_s: f64,
		total_gb: f64,
		errors: u64,
		durations: Durations,
	},
	ContentIdentification {
		meta: RunMeta,
		files: u64,
		files_per_s: f64,
		dirs: u64,
		dirs_per_s: f64,
		total_gb: f64,
		errors: u64,
		durations: Durations,
	},
}
