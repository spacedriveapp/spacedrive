use anyhow::Result;

use crate::core_boot::CoreBoot;
use crate::metrics::ScenarioResult;
use crate::recipe::Recipe;

#[async_trait::async_trait]
pub trait Scenario {
    fn name(&self) -> &'static str;
    fn describe(&self) -> &'static str;
    async fn prepare(&mut self, boot: &CoreBoot, recipe: &Recipe) -> Result<()>;
    async fn run(&mut self, boot: &CoreBoot, recipe: &Recipe) -> Result<Vec<ScenarioResult>>;
}

pub mod indexing_discovery;
pub mod content_identification;
pub mod aggregation;
pub mod registry;

pub use indexing_discovery::IndexingDiscoveryScenario;
pub use content_identification::ContentIdentificationScenario;
pub use aggregation::AggregationScenario;

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
