use anyhow::Result;

use crate::core_boot::CoreBoot;
use crate::metrics::BenchmarkRun;
use crate::recipe::Recipe;

#[async_trait::async_trait]
pub trait Scenario {
	fn name(&self) -> &'static str;
	fn describe(&self) -> &'static str;
	async fn prepare(&mut self, boot: &CoreBoot, recipe: &Recipe) -> Result<()>;
	async fn run(&mut self, boot: &CoreBoot, recipe: &Recipe) -> Result<Vec<BenchmarkRun>>;
	
	/// Set a hardware hint for this scenario (e.g., "nvme", "hdd", "ssd")
	fn set_hardware_hint(&mut self, _hint: Option<String>) {
		// Default implementation does nothing
	}
}

pub mod aggregation;
pub mod content_identification;
pub mod indexing_discovery;
pub mod registry;

pub use aggregation::AggregationScenario;
pub use content_identification::ContentIdentificationScenario;
pub use indexing_discovery::IndexingDiscoveryScenario;

pub fn infer_hardware_label(recipe_name: &str) -> Option<String> {
	let r = recipe_name.to_lowercase();
	if r.starts_with("nvme_") {
		Some("Internal NVMe SSD".to_string())
	} else if r.starts_with("hdd_") {
		Some("External HDD (USB 3.0)".to_string())
	} else if r.contains("nas") {
		Some("Network Attached Storage (1Gbps)".to_string())
	} else if r.contains("usb") {
		Some("External USB 3.2 SSD".to_string())
	} else {
		None
	}
}

/// Convert a hardware hint (like "nvme", "hdd") to a full hardware label
pub fn hardware_hint_to_label(hint: &str) -> Option<String> {
	match hint.to_lowercase().as_str() {
		"nvme" => Some("Internal NVMe SSD".to_string()),
		"hdd" => Some("External HDD (USB 3.0)".to_string()),
		"ssd" => Some("External SSD (USB 3.0)".to_string()),
		"nas" => Some("Network Attached Storage (1Gbps)".to_string()),
		"usb" => Some("External USB 3.2 SSD".to_string()),
		_ => None,
	}
}
